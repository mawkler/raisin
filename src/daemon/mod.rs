//! The long-running half of raisin: it owns the switcher window, listens for
//! the keys it asked Hyprland to bind, and decides what each one means.

mod controller;
mod ipc;
mod overlay;

use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use gtk4::glib;

use crate::bindings::{self, Target};
use crate::compositor::Compositor as _;
use crate::compositor::integrations::hyprland::{
    self, Binds, CANCEL_EVENT, CONFIRM_EVENT, SWITCH_EVENT,
};
use controller::{Controller, Effect, Event};
use overlay::Overlay;

pub(crate) use ipc::switch;

/// How long Super has to stay held before the switcher appears. Long enough
/// that a tap never puts anything on screen, short enough to feel immediate
/// when the user does mean to look.
const REVEAL_DELAY: Duration = Duration::from_millis(90);

/// Runs the switcher until it's asked to stop.
///
/// # Errors
///
/// Returns an error if Hyprland isn't running, if another daemon already is,
/// or if GTK, the window or the keybinds couldn't be set up.
pub(crate) fn run() -> Result<()> {
    let compositor = hyprland::Compositor;

    anyhow::ensure!(
        compositor.is_running(),
        "raisin's switcher only supports Hyprland, which doesn't appear to be running"
    );

    // Claimed before anything else, so a second daemon fails fast instead of
    // fighting over the keybinds.
    let instance = ipc::Instance::acquire()?;

    gtk4::init().context("failed to initialise GTK")?;

    let binds = Rc::new(Binds::install().context("failed to install raisin's Hyprland keybinds")?);

    let daemon = Rc::new(Daemon {
        controller: RefCell::new(Controller::default()),
        overlay: Overlay::new().context("failed to build the switcher window")?,
        compositor,
        binds: Rc::clone(&binds),
    });

    let main_loop = glib::MainLoop::new(None, false);

    watch_hyprland(&daemon, &main_loop)?;
    watch_clients(&daemon, instance.listener())?;
    quit_on_signal(&main_loop)?;

    main_loop.run();

    // Leave Hyprland the way it was found.
    binds.remove();
    drop(instance);

    Ok(())
}

struct Daemon {
    controller: RefCell<Controller>,
    overlay: Overlay,
    compositor: hyprland::Compositor,
    binds: Rc<Binds>,
}

impl Daemon {
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

                glib::timeout_add_local_once(REVEAL_DELAY, move || {
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
                self.binds.capture_escape();
            }
            Effect::Hide => {
                self.overlay.hide();
                self.binds.release_escape();
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
    fn trigger(self: &Rc<Self>, target: Target) {
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
        "custom" => {
            let Some(key) = data
                .strip_prefix(SWITCH_EVENT)
                .and_then(|key| key.chars().next())
            else {
                return;
            };
            let Some(binding) = bindings::binding(key) else {
                return;
            };

            daemon.trigger(binding.target());
        }
        // A reload wipes keybinds that were added over IPC.
        "configreloaded" => daemon.binds.reinstall(),
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
                    daemon.trigger(Target::new(&app, app_id.as_deref()));
                }
            }
        }
    });

    Ok(())
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
