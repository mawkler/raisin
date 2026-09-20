//! The long-running half of raisin: it owns the switcher window, listens for
//! the keys it asked Hyprland to bind, and decides what each one means.

mod controller;
mod ipc;
mod overlay;

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use gtk4::glib;

use crate::compositor::Compositor as _;
use crate::compositor::integrations::hyprland::{
    self, BACK_EVENT, Binds, CANCEL_EVENT, CONFIRM_EVENT, NEXT_EVENT, PREVIOUS_EVENT, SWITCH_EVENT,
};
use crate::config::{Config, Target};
use crate::switcher::Direction;
use controller::{Controller, Effect, Event};
use overlay::Overlay;

pub(crate) use ipc::switch;

/// How long to let a configuration file settle before reading it. Editors
/// write, rename and truncate in quick succession, and raisin would rather
/// read the result once than read half of it three times.
const SETTLE: Duration = Duration::from_millis(60);

/// Runs the switcher until it's asked to stop.
///
/// # Errors
///
/// Returns an error if Hyprland isn't running, if another daemon already is,
/// or if GTK, the window or the keybinds couldn't be set up.
pub(crate) fn run(path: Option<&Path>) -> Result<()> {
    let config_path = Config::path(path);
    let config = Rc::new(Config::load(path)?);
    let compositor = hyprland::Compositor;

    anyhow::ensure!(
        compositor.is_running(),
        "raisin's switcher only supports Hyprland, which doesn't appear to be running"
    );

    // Claimed before anything else, so a second daemon fails fast instead of
    // fighting over the keybinds.
    let instance = ipc::Instance::acquire()?;

    gtk4::init().context("failed to initialise GTK")?;

    let binds =
        Rc::new(Binds::install(&config).context("failed to install raisin's Hyprland keybinds")?);

    let daemon = Rc::new(Daemon {
        controller: RefCell::new(Controller::default()),
        overlay: Overlay::new(&config.switcher, &config.keys)
            .context("failed to build the switcher window")?,
        compositor,
        binds: RefCell::new(binds),
        config: RefCell::new(config),
        config_path: config_path.clone(),
    });

    let main_loop = glib::MainLoop::new(None, false);

    watch_hyprland(&daemon, &main_loop)?;
    watch_clients(&daemon, instance.listener())?;

    if let Some(path) = &config_path {
        watch_config(&daemon, path);
    }

    quit_on_signal(&main_loop)?;

    main_loop.run();

    // Leave Hyprland the way it was found.
    daemon.binds().remove();
    drop(instance);

    Ok(())
}

struct Daemon {
    controller: RefCell<Controller>,
    overlay: Overlay,
    compositor: hyprland::Compositor,
    binds: RefCell<Rc<Binds>>,
    config: RefCell<Rc<Config>>,
    config_path: Option<PathBuf>,
}

impl Daemon {
    fn config(&self) -> Rc<Config> {
        Rc::clone(&self.config.borrow())
    }

    fn binds(&self) -> Rc<Binds> {
        Rc::clone(&self.binds.borrow())
    }

    /// Reads the configuration file again and takes on what changed.
    ///
    /// A file that doesn't parse leaves the daemon exactly as it was, since a
    /// half-saved file is a normal thing for an editor to leave behind for a
    /// moment.
    fn reload(self: &Rc<Self>) {
        let config = match Config::load(self.config_path.as_deref()) {
            Ok(config) => Rc::new(config),
            Err(error) => {
                eprintln!("raisin: keeping the configuration it had: {error:#}");
                return;
            }
        };

        // A switch in progress was started under the old keys, so it ends here
        // rather than half under each.
        self.handle(Event::Cancel);

        let previous = self.binds();
        previous.remove();

        match Binds::install(&config) {
            Ok(binds) => {
                self.overlay.reconfigure(&config.switcher, &config.keys);
                self.binds.replace(Rc::new(binds));
                self.config.replace(config);
            }
            Err(error) => {
                eprintln!("raisin: the new configuration's keybinds were refused: {error:#}");

                // Put back the ones that were working.
                match Binds::install(&self.config()) {
                    Ok(binds) => {
                        self.binds.replace(Rc::new(binds));
                    }
                    Err(error) => eprintln!("raisin: and its own keybinds are gone too: {error:#}"),
                }
            }
        }
    }
    fn handle(self: &Rc<Self>, event: Event) {
        let effects = self.controller.borrow_mut().handle(event);

        for effect in effects {
            self.apply(effect);
        }
    }

    fn apply(self: &Rc<Self>, effect: Effect) {
        match effect {
            Effect::ScheduleReveal { session } => {
                let daemon = Rc::clone(self);

                glib::timeout_add_local_once(self.config().switcher.delay(), move || {
                    daemon.handle(Event::Reveal { session });
                });
            }
            Effect::Fill => {
                if let Some(session) = self.controller.borrow().session() {
                    self.overlay.fill(session);
                }
            }
            Effect::Highlight => {
                if let Some(session) = self.controller.borrow().session() {
                    self.overlay.highlight(session);
                }
            }
            Effect::Show => {
                self.overlay.show();
                self.binds().capture_session_keys();
            }
            Effect::Hide => {
                self.overlay.hide();
                self.binds().release_session_keys();
            }
            Effect::Focus(window) => {
                if let Err(error) = self.compositor.focus_window(&window) {
                    eprintln!("raisin: {error:#}");
                }
            }
            Effect::Launch(app) => {
                if let Err(error) = self.compositor.launch_application(&app) {
                    eprintln!("raisin: {error:#}");
                }
            }
        }
    }

    /// A mapped key was pressed: take a snapshot of the open windows and let
    /// the controller decide what it means.
    fn trigger(self: &Rc<Self>, target: Target, direction: Direction) {
        let windows = match self.compositor.get_windows() {
            Ok(windows) => windows,
            Err(error) => {
                eprintln!("raisin: {error:#}");
                return;
            }
        };

        let focused = match self.compositor.get_focused_window() {
            Ok(focused) => focused,
            Err(error) => {
                eprintln!("raisin: {error:#}");
                None
            }
        };

        self.handle(Event::Trigger {
            target,
            direction,
            windows,
            focused,
        });
    }
}

/// Follows Hyprland's event stream: the keys raisin asked it to bind arrive
/// here, in the order they were pressed, whatever the window happens to be
/// doing. That ordering is what makes releasing Super reliable — the release
/// can't overtake the key press that started the switch, and neither waits
/// for anything to be drawn.
fn watch_hyprland(daemon: &Rc<Daemon>, main_loop: &glib::MainLoop) -> Result<()> {
    let events = BufReader::new(hyprland::events()?);
    let (sender, receiver) = async_channel::unbounded();

    thread::spawn(move || {
        for line in events.lines() {
            let Ok(line) = line else { break };

            if sender.send_blocking(line).is_err() {
                break;
            }
        }
    });

    let daemon = Rc::clone(daemon);
    let main_loop = main_loop.clone();

    glib::MainContext::default().spawn_local(async move {
        while let Ok(line) = receiver.recv().await {
            on_hyprland_event(&daemon, &line);
        }

        // Hyprland is gone, and so is the point of running.
        main_loop.quit();
    });

    Ok(())
}

fn on_hyprland_event(daemon: &Rc<Daemon>, line: &str) {
    let Some((event, data)) = line.split_once(">>") else {
        return;
    };

    match event {
        "custom" if data == CONFIRM_EVENT => daemon.handle(Event::Confirm),
        "custom" if data == CANCEL_EVENT => daemon.handle(Event::Cancel),
        "custom" if data == NEXT_EVENT => daemon.handle(Event::Cycle {
            direction: Direction::Forward,
        }),
        "custom" if data == PREVIOUS_EVENT => daemon.handle(Event::Cycle {
            direction: Direction::Backward,
        }),
        "custom" => {
            let pressed = [
                (SWITCH_EVENT, Direction::Forward),
                (BACK_EVENT, Direction::Backward),
            ]
            .into_iter()
            .find_map(|(event, direction)| Some((data.strip_prefix(event)?, direction)));

            let Some((key, direction)) = pressed else {
                return;
            };
            let config = daemon.config();
            let Some(target) = key
                .chars()
                .next()
                .and_then(|key| config.target(key))
                .cloned()
            else {
                return;
            };

            daemon.trigger(target, direction);
        }
        // A reload wipes keybinds that were added over IPC.
        "configreloaded" => daemon.binds().reinstall(),
        _ => {}
    }
}

/// Follows the daemon's own socket, where `raisin switch` sends its requests.
fn watch_clients(daemon: &Rc<Daemon>, listener: &UnixListener) -> Result<()> {
    let listener = listener
        .try_clone()
        .context("failed to watch the daemon's socket")?;
    let (sender, receiver) = async_channel::unbounded();

    thread::spawn(move || {
        for client in listener.incoming() {
            let Ok(client) = client else { continue };
            let Some(message) = ipc::receive(client) else {
                continue;
            };

            if sender.send_blocking(message).is_err() {
                break;
            }
        }
    });

    let daemon = Rc::clone(daemon);

    glib::MainContext::default().spawn_local(async move {
        while let Ok(message) = receiver.recv().await {
            match message {
                ipc::Message::Switch { app, app_id } => {
                    daemon.trigger(Target::new(&app, app_id.as_deref()), Direction::Forward);
                }
            }
        }
    });

    Ok(())
}

/// Watches the configuration file, so that saving it is all it takes for the
/// change to be in effect.
fn watch_config(daemon: &Rc<Daemon>, path: &Path) {
    let (Some(directory), Some(name)) = (path.parent(), path.file_name().map(OsString::from))
    else {
        return;
    };

    let mut inotify = match inotify::Inotify::init() {
        Ok(inotify) => inotify,
        Err(error) => {
            eprintln!("raisin: not watching {}: {error}", path.display());
            return;
        }
    };

    // The directory rather than the file: an editor saves by writing a new
    // file and renaming it over the old one, which leaves a watch on the file
    // itself pointing at something nobody will write to again.
    let watching = inotify.watches().add(
        directory,
        inotify::WatchMask::CLOSE_WRITE | inotify::WatchMask::MOVED_TO | inotify::WatchMask::CREATE,
    );

    if let Err(error) = watching {
        eprintln!("raisin: not watching {}: {error}", directory.display());
        return;
    }

    let (sender, receiver) = async_channel::unbounded();

    thread::spawn(move || {
        let mut buffer = [0; 4096];

        loop {
            let Ok(mut events) = inotify.read_events_blocking(&mut buffer) else {
                break;
            };

            if !events.any(|event| event.name == Some(&name)) {
                continue;
            }

            // Let the rest of the editor's writing land, and read it once.
            thread::sleep(SETTLE);
            let mut settled = [0; 4096];
            let _ = inotify.read_events(&mut settled);

            if sender.send_blocking(()).is_err() {
                break;
            }
        }
    });

    let daemon = Rc::clone(daemon);

    glib::MainContext::default().spawn_local(async move {
        while receiver.recv().await.is_ok() {
            daemon.reload();
        }
    });
}

/// Stops on the signals a service manager stops things with, so the keybinds
/// are taken away rather than left behind.
fn quit_on_signal(main_loop: &glib::MainLoop) -> Result<()> {
    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
    ])
    .context("failed to listen for termination signals")?;
    let (sender, receiver) = async_channel::bounded(1);

    thread::spawn(move || {
        if signals.forever().next().is_some() {
            let _ = sender.send_blocking(());
        }
    });

    let main_loop = main_loop.clone();

    glib::MainContext::default().spawn_local(async move {
        let _ = receiver.recv().await;
        main_loop.quit();
    });

    Ok(())
}
