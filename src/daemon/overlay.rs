//! The switcher window: a small dark panel, centred, above everything else.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use gtk4::{gdk, glib, pango};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::compositor::Window;
use crate::config;
use crate::preview::{self, RATIO, Rgb, Thumbnail, Tint};
use crate::switcher::{Row, Session};

/// How big an application's icon is beside its name.
const ICON_SIZE: i32 = 16;

/// How many characters of an application's name a row shows before ellipsising
/// it. Every row's name is given the same width, so this is what that width
/// works out to for the longest of them.
const NAME_WIDTH: i32 = 14;

/// How tall the marker standing in for a window of an application that isn't
/// the one being switched to is. Its width comes from the window.
const PILL: i32 = 24;

/// How long a handover takes: one application's row folding shut while
/// another's folds open, on the curve the compositor animates its own windows
/// and layers with.
const FOLD: f32 = 170.0;

/// And how long after that the rows are put back to how they rest, whether or
/// not the frames got that far.
const FOLDED: Duration = Duration::from_millis(230);

/// How big an application's icon is drawn when it is being read for its
/// colours rather than looked at. Big enough for the colours it is mostly
/// made of to be the colours it looks like.
const SAMPLE: i32 = 48;

/// How big the icon of an application that was just started is drawn at the
/// moment it is most solid.
const STARTING_SIZE: i32 = 96;

/// How far it drifts up as it appears.
const LIFT: f32 = 10.0;

/// How small it starts and how large it ends, against that size. It arrives at
/// its own size and then keeps growing, so that it swells away as it fades
/// rather than standing still while it goes.
const GROW: f32 = 0.85;
const GROWN: f32 = 1.4;

/// How solid the icon already is at the moment it appears.
///
/// It starts part of the way in rather than from nothing: what it is there to
/// say is that something happened just now, and a fade that begins at nothing
/// says it a moment late.
const FAINT: f32 = 0.35;

/// Arriving, and then leaving again, in milliseconds. Quick to arrive and
/// slower to leave: what it says is that something has started, which is
/// worth a glance and no more.
///
/// There is no pause between the two. It doesn't need one: the fade is slow
/// to start, which leaves a moment where the icon is simply there.
const APPEARING: f32 = 70.0;
const LEAVING: f32 = 250.0;

/// How long after that the window comes down regardless.
///
/// Frames only arrive while something is being drawn, so they can't be trusted
/// to end anything: a window that never gets one would sit there for ever.
const SLACK: f32 = 100.0;

/// What is shown for an application the icon theme has nothing for. Something
/// has to appear, or the animation says nothing at all.
const FALLBACK_ICON: &str = "application-x-executable";

/// How strongly a window's own colours are laid over the panel.
///
/// The marker's title sits on top of them, so how much of a window's colour
/// can be shown depends on how light that colour is allowed to be. They are
/// held under a ceiling before they arrive, which leaves the title a contrast
/// of 4.5 against the palest window there can be — and room to show this much
/// of every other one.
const TINT: &str = "0.35";

const STYLE: &str = "
window.raisin,
window.raisin > widget {
    background-color: transparent;
}

.panel {
    margin: 18px;
    padding: 18px 16px 14px 16px;
    border: 1px solid alpha(#ffffff, 0.08);
    border-radius: 18px;
    background-color: alpha(#15171c, 0.97);
    box-shadow: 0 20px 50px alpha(#000000, 0.55);
}

.title-bar {
    padding: 0 4px 14px 4px;
}

.heading {
    color: #eef1f7;
    font-size: 15px;
    font-weight: 600;
}

/* The window the switch would land on, beside the application's name. */
.subject {
    color: #78819a;
    font-size: 13px;
}

.group {
    color: #78819a;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1px;
}

/* One application per row: its name, then its windows. */
.row {
    padding: 2px 0;
}

.marker {
    color: #c4cad8;
    font-size: 12px;
    padding: 0 6px;
}

/* The line above them, which is there to say that what is below it is a
   different kind of thing from what is above. */
.between {
    min-height: 1px;
    margin: 10px 0 2px 0;
    background-color: alpha(#ffffff, 0.09);
}

/* An application with nothing open, there to show its key and no more; and
   the hints, which say the same thing every time. Both are worth a glance
   and neither is worth the eye that the windows themselves are worth. */
.absent .group,
.absent .keycap,
.absent .app-icon,
.footer {
    opacity: 0.4;
}

.tile {
    padding: 6px;
    border-radius: 10px;
    outline: none;
}

.tile.selected {
    background-color: alpha(#6b8cff, 0.22);
    box-shadow: inset 0 0 0 1px alpha(#86a4ff, 0.65);
}

.title {
    color: #c4cad8;
    font-size: 12px;
    padding: 6px 2px 0 2px;
}

.tile.selected .title {
    color: #ffffff;
}

.thumbnail {
    border: 1px solid alpha(#ffffff, 0.10);
    border-radius: 6px;
    background-color: alpha(#000000, 0.25);
}

/* One window of an application that isn't the one being switched to. The
   thumbnail's frame is nearly black, which reads as a window showing
   something; these show nothing, so they are a plain grey instead. */
.pill {
    border-radius: 4px;
    border-color: alpha(#ffffff, 0.14);
    background-color: alpha(#8f98ac, 0.20);
}

/* Stands in for a window the compositor wouldn't copy. Faint, so a tile that
   has its own picture never looks like one that hasn't. */
.standin {
    opacity: 0.35;
}

/* Held off the heading beside it, and level with it. */
.footer {
    padding: 0 0 0 14px;
}

/* What each key does, in the grey the names of the applications with nothing
   open wear, so that the two rows of keys read alike. */
.hint {
    color: #78819a;
    font-size: 11px;
}

.keycap {
    color: #b7bfd0;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border: 1px solid alpha(#ffffff, 0.10);
    border-radius: 7px;
    background-color: alpha(#ffffff, 0.06);
}

scrollbar {
    background-color: transparent;
}
";

/// What a desktop entry says about an application.
struct Entry {
    /// The entry's own file name, without `.desktop`.
    entry: String,
    /// The command it runs, without its path.
    runs: String,
    /// The window class it says its windows carry. This is the field made for
    /// recognising an application's windows, and the one worth trusting: Zen's
    /// entry is `zen-beta`, Beeper's is `beepertexts`, and neither is what
    /// their windows are called.
    class: String,
    name: String,
    /// The icon theme's name for it, which is again its own: Beeper's windows
    /// say `Beeper` and its icon is `beepertexts`.
    icon: String,
}

/// An application that is configured but has no windows open. It still gets a
/// row, so its key is somewhere to be seen rather than only in the
/// configuration file.
pub(crate) struct Absent {
    /// What the `[names]` table would call it, keyed the way groups are.
    pub(crate) app_id: String,
    /// What to call it when the table doesn't.
    pub(crate) app: String,
    pub(crate) trigger: String,
}

/// What a capture of a window leaves behind: the picture itself, and the two
/// colours it is mostly made of.
struct Capture {
    texture: gdk::MemoryTexture,
    tint: Option<Tint>,
}

/// A tile's thumbnail: the picture a capture goes into, and the icon shown in
/// its place until one arrives — or for good, when none ever does.
struct Thumbnailed {
    picture: gtk4::Picture,
    standin: Option<gtk4::Image>,
}

impl Thumbnailed {
    /// Puts a capture in the tile, and takes the icon standing in for it away.
    fn show(&self, texture: &gdk::MemoryTexture) {
        self.picture.set_paintable(Some(texture));

        if let Some(standin) = &self.standin {
            standin.set_visible(false);
        }
    }
}

pub(crate) struct Overlay {
    window: gtk4::Window,
    heading: gtk4::Label,
    /// The window a release of Super would land on.
    subject: gtk4::Label,
    /// The applications, side by side.
    strip: gtk4::Box,
    /// Each window's tile, by the window it stands for, so that the highlight
    /// can move without rebuilding anything.
    tiles: RefCell<HashMap<String, gtk4::Box>>,
    selected: RefCell<Option<gtk4::Box>>,
    panel: gtk4::Box,
    /// The line the heading and the key hints share, kept so that the hints
    /// can be built again when the keys they name change.
    title: gtk4::Box,
    /// The switcher and the just-started icon, one of which the window shows.
    faces: gtk4::Stack,
    /// The face that says an application is starting, and the icon on it.
    splash: gtk4::Box,
    starting: gtk4::Image,
    /// The animation in progress, if there is one: kept so that a switch can
    /// cut it short, and so that it is only ever taken down once — whoever
    /// gets here first takes it, and the other finds nothing.
    playing: Rc<RefCell<Option<Playing>>>,
    scroll: gtk4::ScrolledWindow,
    footer: RefCell<gtk4::Box>,
    /// The hints among them that name the key of whatever is being switched
    /// to, which changes with every switch.
    cycling: RefCell<Cycling>,
    /// Where each window's thumbnail goes once it has been captured, by the
    /// identifier the capture comes back with.
    thumbnails: RefCell<HashMap<String, Thumbnailed>>,
    /// What each window last looked like. A picture only holds its thumbnail
    /// until the strip is rebuilt; keeping the texture as well is what lets a
    /// switcher open showing windows rather than empty boxes, until the
    /// captures for this session come in behind it.
    captures: RefCell<HashMap<String, Capture>>,
    /// Each marker on screen, so a capture arriving after the strip was built
    /// can still colour the one it belongs to.
    pills: RefCell<HashMap<String, gtk4::Box>>,
    /// The gradients, in a sheet of their own: a colour taken from a window
    /// can't be written in the stylesheet that ships with the program.
    tints: gtk4::CssProvider,
    /// Which of them that sheet currently holds, so it is only rewritten when
    /// something new turns up.
    rules: RefCell<BTreeSet<Tint>>,
    /// Which application the strip was last built for, so that building it
    /// again can tell a switch from a first showing.
    showing: RefCell<Option<String>>,
    /// A handover in progress, kept so that a switch arriving in the middle of
    /// one can call it off — and cleared by the handover itself when it ends,
    /// so that what is kept is only ever a timer that has yet to fire.
    /// Removing one that already has is an error, not a no-op.
    folding: Rc<RefCell<Option<Folding>>>,
    /// What each application's icon is made of, by `app_id`, for the windows
    /// the compositor won't give a picture of. Kept because an icon doesn't
    /// change while the daemon runs, and `None` is worth keeping too.
    palettes: RefCell<HashMap<String, Option<Tint>>>,
    previews: Cell<config::Previews>,
    icons: Cell<bool>,
    /// What to call each application, by `app_id`, for the ones the user would
    /// rather name themselves.
    names: RefCell<BTreeMap<String, String>>,
    /// Every desktop entry on the system.
    ///
    /// Read once and kept: walking every entry on the system is far too much
    /// to do while a switch is waiting to appear.
    entries: OnceCell<Vec<Entry>>,
}

impl Overlay {
    /// Builds the window, ready to be put on screen later.
    pub(crate) fn new(
        switcher: &config::Switcher,
        keys: &config::Keys,
        previews: &config::Previews,
        names: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let tints = load_style().context("failed to load the switcher's stylesheet")?;

        let window = gtk4::Window::new();
        window.add_css_class("raisin");
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace(Some("raisin"));
        // Without anchors the compositor centres the window.
        //
        // The overlay deliberately takes no keyboard focus: it would otherwise
        // become the focused surface, and Hyprland would hand focus back to
        // whatever was focused before when it disappears — undoing the switch.
        // Escape arrives through a keybind instead, like every other key the
        // switcher listens to.
        window.set_keyboard_mode(KeyboardMode::None);

        let heading = gtk4::Label::new(None);
        heading.add_css_class("heading");

        // The window the switch would land on, beside the application's name:
        // with one row of thumbnails among rows of markers, the heading is
        // where you read what you are actually about to get.
        let subject = gtk4::Label::new(None);
        subject.add_css_class("subject");
        line(&subject);

        // What the keys do, at the end of the line the heading is on: it is
        // nearly the same every time, so it belongs where the eye passes over
        // it rather than on a line of its own.
        let (footer, cycling) = footer(keys);

        let title = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        title.add_css_class("title-bar");
        title.append(&heading);
        title.append(&subject);
        title.append(&footer);

        let strip = gtk4::Box::new(gtk4::Orientation::Vertical, 10);

        // One row per application, stacked downwards. The panel is as wide as
        // it is configured to be rather than as wide as its contents: a row
        // with more windows than fit scrolls within itself, so the names stay
        // where they are instead of sliding off the side.
        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_child(Some(&strip));
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        let (screen_width, screen_height) = screen();
        scroll.set_size_request(switcher.width.pixels(screen_width), -1);
        scroll.set_max_content_height(switcher.max_height.pixels(screen_height));

        let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        panel.add_css_class("panel");
        panel.append(&title);
        panel.append(&scroll);

        // The window shows one of two things: the switcher, or the icon of an
        // application that was just started. Swapping which keeps the one
        // window, with its layer-shell setup and its warm renderer, rather
        // than building a second one for a glance.
        let starting = gtk4::Image::new();
        starting.add_css_class("starting");
        // The icon grows for as long as it is on screen, but the room it is
        // drawn in doesn't: holding that at the size it ends at is half of
        // what keeps the window from changing size while the animation runs.
        starting.set_size_request(biggest(), biggest());

        // The face that says an application is starting has no chrome of its
        // own: the application's own logo is the whole of what it has to say.
        let splash = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        splash.add_css_class("splash");
        splash.append(&starting);
        // Room is kept below the icon for it to drift up through, so that the
        // face never changes size. This makes up the difference above it, so
        // that the icon rests in the middle of the screen rather than above
        // the middle of it.
        #[allow(clippy::cast_possible_truncation)]
        splash.set_margin_top(LIFT as i32);

        let faces = gtk4::Stack::new();
        faces.add_child(&panel);
        faces.add_child(&splash);
        // Each face is as big as it needs to be rather than as big as the
        // larger of the two: the splash is a chip around an icon, and a stack
        // left to itself would paint it across the whole width of the panel.
        faces.set_hhomogeneous(false);
        faces.set_vhomogeneous(false);

        window.set_child(Some(&faces));

        // Building the renderer costs a few hundred milliseconds, and paying
        // it on the first switch would hold up the main loop just as the user
        // lets go of Super. Realising the window does that work now: it builds
        // the surface and its renderer without putting anything on screen,
        // which only happens once something is drawn into it.
        gtk4::prelude::WidgetExt::realize(&window);

        Ok(Self {
            window,
            heading,
            subject,
            title,
            strip,
            tiles: RefCell::new(HashMap::new()),
            selected: RefCell::new(None),
            panel,
            faces,
            splash,
            starting,
            playing: Rc::new(RefCell::new(None)),
            scroll,
            footer: RefCell::new(footer),
            cycling: RefCell::new(cycling),
            thumbnails: RefCell::new(HashMap::new()),
            captures: RefCell::new(HashMap::new()),
            pills: RefCell::new(HashMap::new()),
            tints,
            rules: RefCell::new(BTreeSet::new()),
            showing: RefCell::new(None),
            folding: Rc::new(RefCell::new(None)),
            palettes: RefCell::new(HashMap::new()),
            previews: Cell::new(*previews),
            icons: Cell::new(switcher.icons),
            names: RefCell::new(names.clone()),
            entries: OnceCell::new(),
        })
    }

    /// Takes on a configuration that changed while the daemon was running.
    pub(crate) fn reconfigure(
        &self,
        switcher: &config::Switcher,
        keys: &config::Keys,
        previews: &config::Previews,
        names: &BTreeMap<String, String>,
    ) {
        let (screen_width, screen_height) = screen();
        self.scroll
            .set_size_request(switcher.width.pixels(screen_width), -1);
        self.scroll
            .set_max_content_height(switcher.max_height.pixels(screen_height));
        self.previews.set(*previews);
        self.icons.set(switcher.icons);
        self.names.replace(names.clone());

        // A texture captured at the old width would be wider than the frame
        // asks for, and a picture takes the room its texture wants, so keeping
        // these would widen the tiles until fresh captures arrived.
        self.captures.borrow_mut().clear();

        // The footer names the keys, so it's rebuilt rather than edited.
        let (footer, cycling) = footer(keys);
        self.title.remove(&*self.footer.borrow());
        self.title.append(&footer);
        self.footer.replace(footer);
        self.cycling.replace(cycling);
    }

    /// Lays out every open window, its application's key beside the
    /// application's name, with the switch's own application named at the top.
    ///
    /// Every window gets a thumbnail, and one already captured is put back
    /// straight away, so rebuilding the strip doesn't empty it.
    pub(crate) fn fill(
        &self,
        session: &Session,
        triggers: &HashMap<String, String>,
        absent: &[Absent],
    ) {
        self.set_heading(session);
        self.cycling
            .borrow()
            .on(triggers.get(session.group()).map(String::as_str));

        let previews = self.previews.get();

        // A switch made while the panel is up hands it from one application to
        // another, which is worth watching rather than blinking: the row being
        // left folds shut as the row arrived at folds open. There is nothing
        // to hand over on the first fill of a switch, when the window is still
        // off screen, or when there are no thumbnails to fold away.
        self.unfold();
        let handover = self
            .showing
            .replace(Some(session.group().to_owned()))
            .filter(|showing| showing != session.group())
            .filter(|_| self.window.is_visible() && previews.enabled);

        while let Some(block) = self.strip.first_child() {
            self.strip.remove(&block);
        }
        self.selected.replace(None);
        self.tiles.borrow_mut().clear();
        self.thumbnails.borrow_mut().clear();
        self.pills.borrow_mut().clear();

        // Every row's name takes the same width, so the windows all start in
        // the same place however long the applications are called.
        let names = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        // Gathered before anything is built, because the order the rows go in
        // isn't the order they arrive in.
        let mut groups: Vec<(&str, String, Vec<&Window>)> = Vec::new();

        for row in session.rows() {
            match row {
                Row::Group { app_id, name } => {
                    groups.push((app_id, self.label(app_id, None, name), Vec::new()));
                }
                Row::Window { window, .. } => {
                    if let Some((.., windows)) = groups.last_mut() {
                        windows.push(window);
                    }
                }
            }
        }

        // In the order of the keys that reach them, which is the order they
        // are learned in. One with no key of its own can only be reached by
        // walking, so it goes last.
        groups.sort_by_key(|(app_id, ..)| {
            let key = triggers.get(*app_id);

            (key.is_none(), key.cloned().unwrap_or_default().to_lowercase())
        });

        let mut handing: Vec<Morph> = Vec::new();

        for (app_id, name, group) in &groups {
            // Only the application being switched to shows its windows in
            // full. Every other window is one marker, the shape of the window
            // it stands for, so the row is a line tall.
            let targeted = *app_id == session.group();
            // The two rows a switch hands the panel between carry both faces,
            // and are shown the one they are coming from: a fold is a row
            // changing which of the two it is, which it can only do if it has
            // both.
            let folding = handover.as_deref() == Some(*app_id);

            let moving = (handover.is_some() && (targeted || folding)).then(|| {
                // What it moves through, and what it settles into: the first
                // is one widget per window that changes height, the second is
                // built by the same code as every other row, so that what the
                // movement leaves behind is an ordinary one.
                let (face, morphing) = self.morph_face(app_id, group, previews);
                let settled: gtk4::Widget = if targeted {
                    self.open_face(group, previews).upcast()
                } else {
                    self.shut_face(app_id, group, previews).upcast()
                };

                (face, morphing, settled)
            });

            let windows: gtk4::Widget = if let Some((face, ..)) = &moving {
                face.clone().upcast()
            } else if targeted {
                self.open_face(group, previews).upcast()
            } else {
                self.shut_face(app_id, group, previews).upcast()
            };

            // A row of windows scrolls sideways on its own when there are more
            // of them than the panel is wide.
            let sideways = gtk4::ScrolledWindow::new();
            sideways.set_child(Some(&windows));
            sideways.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
            sideways.set_propagate_natural_height(true);
            sideways.set_hexpand(true);

            if let Some((face, morphing, settled)) = moving {
                #[allow(clippy::cast_possible_truncation)]
                let tall = previews.height as i32;

                // How tall the row stands when it is open, asked of the face
                // itself rather than added up: it holds a title as well.
                for one in &morphing {
                    one.frame.set_size_request(one.width, tall);
                }
                let open = face.measure(gtk4::Orientation::Vertical, -1).1;

                // A row that says its size follows its contents can never be
                // shorter than they are, and the whole movement is the row
                // being shorter than they are.
                sideways.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::External);

                let morph = Morph {
                    sideways: sideways.clone(),
                    settled,
                    windows: morphing,
                    open,
                    tall,
                    opening: targeted,
                };

                morph.draw(0.0);
                handing.push(morph);
            }

            // The icon is still looked up by `app_id`: it is what the desktop
            // entry is named after, not what the window calls itself.
            let header = group_header(
                name,
                app_id,
                self.icon_name(app_id).as_deref(),
                triggers.get(*app_id).map(String::as_str),
                self.icons.get(),
                Some(NAME_WIDTH),
            );
            names.add_widget(&header);

            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
            row.add_css_class("row");
            row.append(&header);
            row.append(&sideways);

            self.strip.append(&row);
        }

        // Applications with nothing open share one line between them: they are
        // there so their keys can be seen, which takes a name and no more.
        // Whatever doesn't fit is cut off rather than wrapping.
        if !absent.is_empty() {
            let mut waiting: Vec<_> = absent
                .iter()
                .map(|application| {
                    (
                        self.label(
                            &application.app_id,
                            Some(&application.app),
                            &application.app,
                        ),
                        application,
                    )
                })
                .collect();
            waiting.sort_by_key(|(_, application)| application.trigger.to_lowercase());

            let chips = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);

            for (name, application) in &waiting {
                chips.append(&group_header(
                    name,
                    &application.app_id,
                    self.icon_name(&application.app_id).as_deref(),
                    Some(&application.trigger),
                    self.icons.get(),
                    None,
                ));
            }

            let cut_off = gtk4::ScrolledWindow::new();
            cut_off.set_child(Some(&chips));
            cut_off.set_policy(gtk4::PolicyType::External, gtk4::PolicyType::Never);
            cut_off.set_propagate_natural_height(true);
            // Without this the scroller asks for no width at all and clips
            // every name away.
            cut_off.set_hexpand(true);

            // A line to say that the row below it is a different kind of
            // thing: keys for what could be opened, rather than windows that
            // are. There is nothing to separate it from if nothing is open.
            if self.strip.first_child().is_some() {
                let between = gtk4::Separator::new(gtk4::Orientation::Horizontal);
                between.add_css_class("between");
                self.strip.append(&between);
            }

            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
            row.add_css_class("row");
            row.add_css_class("absent");
            row.append(&cut_off);

            self.strip.append(&row);
        }

        self.fold(handing);

        // Windows that have since closed would otherwise be remembered for as
        // long as the daemon runs.
        //
        // Every open window counts, not only the ones showing a thumbnail: an
        // application that isn't the one being switched to is a row of markers
        // now, and forgetting its captures here would mean taking them again
        // the moment the switch pointed back at it.
        let open: HashSet<&str> = groups
            .iter()
            .flat_map(|(.., windows)| windows.iter())
            .map(|window| window.identifier.as_str())
            .collect();

        self.captures
            .borrow_mut()
            .retain(|identifier, _| open.contains(identifier.as_str()));
    }

    /// Names the application the switch now points at, and the window it
    /// would land on.
    pub(crate) fn set_heading(&self, session: &Session) {
        let name = self.label(session.group(), None, session.label());
        let window = session.selected_window();
        let title = if window.title.is_empty() {
            &window.app_id
        } else {
            &window.title
        };

        self.heading.set_text(&format!("Switch to {name}"));
        self.subject.set_text(title);
    }

    /// An application's windows as thumbnails, which is how the one being
    /// switched to is shown.
    fn open_face(&self, group: &[&Window], previews: config::Previews) -> gtk4::Box {
        let windows = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

        for window in group {
            self.append_tile(&windows, window, previews);
        }

        windows
    }

    /// The same windows as markers, each the shape of the window it stands
    /// for, which is how every other application is shown: a line tall.
    fn shut_face(
        &self,
        app_id: &str,
        group: &[&Window],
        previews: config::Previews,
    ) -> gtk4::Box {
        let windows = gtk4::Box::new(gtk4::Orientation::Horizontal, 5);

        for window in group {
            let title = if window.title.is_empty() {
                &window.app_id
            } else {
                &window.title
            };

            let pill = pill(title, shape(window.size), previews.height);

            let captured = self
                .captures
                .borrow()
                .get(&window.identifier)
                .and_then(|capture| capture.tint);

            // A window the compositor won't copy still belongs to an
            // application, and that has an icon: its colours are the ones the
            // window would be recognised by anyway.
            if let Some(tint) = captured.or_else(|| self.palette(app_id)) {
                self.rule(tint);
                wear(&pill, tint);
            }

            windows.append(&pill);
            self.pills
                .borrow_mut()
                .insert(window.identifier.clone(), pill);
        }

        windows
    }

    /// The windows of a row that is folding or opening, each as one widget
    /// that is a marker at one end of the movement and a thumbnail at the
    /// other.
    fn morph_face(
        &self,
        app_id: &str,
        group: &[&Window],
        previews: config::Previews,
    ) -> (gtk4::Box, Vec<Morphing>) {
        let windows = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let mut moving = Vec::new();

        for window in group {
            let title = if window.title.is_empty() {
                &window.app_id
            } else {
                &window.title
            };

            let capture = self.captures.borrow().get(&window.identifier).map(
                |Capture { texture, tint }| (texture.clone(), *tint),
            );
            let (texture, captured) = match &capture {
                Some((texture, tint)) => (Some(texture), *tint),
                None => (None, None),
            };

            let one = morphing(title, shape(window.size), previews.height, texture);

            if let Some(tint) = captured.or_else(|| self.palette(app_id)) {
                self.rule(tint);
                wear(&one.frame, tint);
            }

            windows.append(&one.tile);
            moving.push(one);
        }

        (windows, moving)
    }

    /// Hands the panel from one application's row to another's, once both have
    /// been drawn the way they were before the switch.
    ///
    /// A stack animates when the child it shows changes, and a stack built and
    /// pointed at a child in the same breath has nothing to change from — so
    /// the fold waits for a frame to be drawn. The first tick comes before the
    /// rows have been given their size, which is the size the fold has to
    /// start from; the second comes after.
    fn fold(&self, rows: Vec<Morph>) {
        if rows.is_empty() {
            return;
        }

        let rows = Rc::new(rows);
        let drawing = Rc::clone(&rows);
        // The clock is read rather than the wall time, and the first frame is
        // whenever the compositor gets round to drawing one.
        let started: Cell<Option<i64>> = Cell::new(None);

        let ticking = self.window.add_tick_callback(move |_, clock| {
            let now = clock.frame_time();
            let start = started.get().unwrap_or_else(|| {
                started.set(Some(now));
                now
            });
            #[allow(clippy::cast_precision_loss)]
            let elapsed = (now - start) as f32 / 1_000.0;

            for row in drawing.iter() {
                row.draw(quintic(elapsed / FOLD));
            }

            glib::ControlFlow::Continue
        });

        // The timer is what ends it, rather than the last frame: it runs
        // whether or not anything was ever drawn, and a row left halfway is a
        // row the next switch would animate out of the wrong place.
        let folding = Rc::clone(&self.folding);
        let settling = Rc::clone(&rows);
        let ending = glib::timeout_add_local_once(FOLDED, move || {
            if let Some(Folding { ticking, .. }) = folding.take() {
                ticking.remove();
            }

            for row in settling.iter() {
                row.settle();
            }
        });

        self.folding.replace(Some(Folding { ticking, ending }));
    }

    /// Calls off a handover, putting the rows it was moving back to how they
    /// rest. They are usually about to be thrown away, but not always: a
    /// switch can be cancelled as well as replaced.
    fn unfold(&self) {
        if let Some(Folding { ticking, ending }) = self.folding.take() {
            ticking.remove();
            ending.remove();
        }
    }

    /// Puts a window's tile into `row`, showing whatever has been captured of
    /// it already.
    fn append_tile(&self, row: &gtk4::Box, window: &Window, previews: config::Previews) {
        // A window without a title is better named by its application than by
        // an empty tile.
        let title = if window.title.is_empty() {
            &window.app_id
        } else {
            &window.title
        };
        let preview =
            (previews.enabled && !window.identifier.is_empty()).then_some(previews.height);

        let (tile, thumbnail) = tile(
            title,
            preview,
            shape(window.size),
            &window.app_id,
            self.icon_name(&window.app_id).as_deref(),
        );

        if let Some(thumbnail) = thumbnail {
            if let Some(capture) = self.captures.borrow().get(&window.identifier) {
                thumbnail.show(&capture.texture);
            }

            self.thumbnails
                .borrow_mut()
                .insert(window.identifier.clone(), thumbnail);
        }

        row.append(&tile);
        self.tiles.borrow_mut().insert(window.id.clone(), tile);
    }

    /// Whether this window has been captured at some point, and so has
    /// something to show while a fresh capture is taken.
    pub(crate) fn captured(&self, identifier: &str) -> bool {
        self.captures.borrow().contains_key(identifier)
    }

    /// The desktop entry for an application, if one of them is plainly about
    /// it.
    ///
    /// The window class it declares comes first, then its own file name, then
    /// the command it runs: a system can carry several entries running one
    /// command, and the extras tend to say less about the application.
    fn entry(&self, app_id: &str, cmd: Option<&str>) -> Option<&Entry> {
        let entries = self.entries.get_or_init(desktop_entries);
        let wanted: Vec<String> = std::iter::once(app_id)
            .chain(cmd)
            .map(str::to_lowercase)
            .collect();
        let matches = |field: &String| !field.is_empty() && wanted.contains(field);

        entries
            .iter()
            .find(|entry| matches(&entry.class))
            .or_else(|| entries.iter().find(|entry| matches(&entry.entry)))
            .or_else(|| entries.iter().find(|entry| matches(&entry.runs)))
    }

    /// What the icon theme calls an application's icon, when its desktop entry
    /// says something other than the window class.
    fn icon_name(&self, app_id: &str) -> Option<String> {
        self.entry(app_id, None)
            .map(|entry| entry.icon.clone())
            .filter(|icon| !icon.is_empty())
    }

    /// What to call an application on screen.
    ///
    /// What the user called it wins. Then what its desktop entry calls it,
    /// which is the application's own name rather than anything inferred from
    /// it — `Files` rather than `org.gnome.Nautilus`, and `Gram` rather than
    /// whichever document its first window happens to have open.
    ///
    /// `otherwise` is the last resort, for an application no entry describes:
    /// the name its windows opened under when it is running, and the command
    /// that would start it when it isn't.
    fn label(&self, app_id: &str, cmd: Option<&str>, otherwise: &str) -> String {
        if let Some(name) = self.names.borrow().get(app_id) {
            return name.clone();
        }

        self.entry(app_id, cmd)
            .map_or_else(|| otherwise.to_owned(), |entry| entry.name.clone())
    }

    /// Remembers what a window looks like, and shows it if the window still
    /// has a tile on screen.
    pub(crate) fn set_thumbnail(&self, thumbnail: Thumbnail) {
        // A capture asked for before the configured width shrank would take
        // more room than its frame allows, widening the tile it sits in.
        if thumbnail.height > self.previews.get().height {
            return;
        }

        self.tint(&thumbnail.identifier, thumbnail.tint);

        let width = thumbnail.width;
        let pixels = glib::Bytes::from_owned(thumbnail.pixels);
        let texture = gdk::MemoryTexture::new(
            width as i32,
            thumbnail.height as i32,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &pixels,
            width as usize * 4,
        );

        if let Some(thumbnailed) = self.thumbnails.borrow().get(&thumbnail.identifier) {
            thumbnailed.show(&texture);
        }

        self.captures.borrow_mut().insert(
            thumbnail.identifier,
            Capture {
                texture,
                tint: thumbnail.tint,
            },
        );
    }

    /// Colours a window's marker, if it has one on screen.
    ///
    /// A capture can land at any time, including long after the strip it
    /// belongs to was built, so the marker is found rather than passed in.
    fn tint(&self, identifier: &str, tint: Option<Tint>) {
        let Some(tint) = tint else {
            return;
        };

        // Worth keeping even with no marker to put it on, for the next time
        // this window has one.
        self.rule(tint);

        if let Some(pill) = self.pills.borrow().get(identifier) {
            wear(pill, tint);
        }
    }

    /// What an application's icon is mostly made of.
    fn palette(&self, app_id: &str) -> Option<Tint> {
        if let Some(known) = self.palettes.borrow().get(app_id) {
            return *known;
        }

        let palette = icon_named(app_id, self.icon_name(app_id).as_deref())
            .and_then(|name| self.drawn(&name))
            .and_then(|memory| preview::colours(&memory));

        self.palettes.borrow_mut().insert(app_id.to_owned(), palette);

        palette
    }

    /// Draws an icon into memory, four bytes to a pixel.
    ///
    /// The window's own renderer does the drawing, which is the only way to
    /// see what an icon is made of: the theme answers with something that
    /// knows how to draw itself rather than with any pixels.
    fn drawn(&self, name: &str) -> Option<Vec<u8>> {
        let theme = gtk4::IconTheme::for_display(&gdk::Display::default()?);
        let icon = theme.lookup_icon(
            name,
            &[],
            SAMPLE,
            1,
            gtk4::TextDirection::None,
            gtk4::IconLookupFlags::empty(),
        );

        let snapshot = gtk4::Snapshot::new();
        icon.snapshot(&snapshot, f64::from(SAMPLE), f64::from(SAMPLE));

        let texture = self
            .window
            .native()?
            .renderer()?
            .render_texture(&snapshot.to_node()?, None);

        #[allow(clippy::cast_sign_loss)]
        let (width, height) = (texture.width() as usize, texture.height() as usize);
        let mut memory = vec![0; width * height * 4];
        texture.download(&mut memory, width * 4);

        Some(memory)
    }

    /// Makes sure the sheet of gradients holds this one, rewriting it only if
    /// it didn't.
    fn rule(&self, tint: Tint) {
        if !self.rules.borrow_mut().insert(tint) {
            return;
        }

        let sheet: String = self.rules.borrow().iter().copied().map(rule).collect();

        self.tints.load_from_string(&sheet);
    }

    /// Marks the window that a release of Super would focus, scrolling it
    /// into view.
    pub(crate) fn highlight(&self, session: &Session) {
        if let Some(previous) = self.selected.replace(None) {
            previous.remove_css_class("selected");
        }

        let tile = self
            .tiles
            .borrow()
            .get(&session.selected_window().id)
            .cloned();
        let Some(tile) = tile else {
            return;
        };

        self.set_heading(session);
        tile.add_css_class("selected");
        tile.grab_focus();
        self.selected.replace(Some(tile));
    }

    pub(crate) fn show(&self) {
        // A switch takes the window back from any animation still playing on
        // it: whatever was starting, the user has moved on. The window goes
        // down first, so that the change of face lands while it is off screen
        // rather than as a jump in size.
        if self.playing.borrow().is_some() {
            self.window.set_visible(false);
        }

        self.stop();
        self.faces.set_visible_child(&self.panel);
        self.window.present();
    }

    /// Says that an application has been started, by showing its icon for
    /// about as long as it takes to notice.
    ///
    /// Nothing waits for this. The application was launched before the first
    /// frame was drawn, and a switch that arrives mid-animation simply takes
    /// the window back.
    pub(crate) fn starting(&self, app_id: &str, cmd: &str) {
        let named = self
            .entry(app_id, Some(cmd))
            .map(|entry| entry.icon.clone())
            .filter(|icon| !icon.is_empty());
        let name = icon_named(app_id, named.as_deref())
            .or_else(|| icon_named(cmd, None))
            // Something has to appear, or the animation says nothing at all.
            .unwrap_or_else(|| FALLBACK_ICON.to_owned());

        self.stop();
        self.starting.set_icon_name(Some(&name));
        // The window goes up with the first frame already drawn on it, rather
        // than with whatever the animation before it left behind.
        if let Some(first) = frame(0.0) {
            draw(&self.splash, &self.starting, first);
        }
        self.faces.set_visible_child(&self.splash);
        self.window.present();

        let splash = self.splash.clone();
        let image = self.starting.clone();
        // The clock is read rather than the wall time, and the first frame is
        // whenever the compositor gets round to drawing one — which is not
        // necessarily now.
        let started: Cell<Option<i64>> = Cell::new(None);

        let ticking = self.splash.add_tick_callback(move |_, clock| {
            let now = clock.frame_time();
            let start = started.get().unwrap_or_else(|| {
                started.set(Some(now));
                now
            });
            #[allow(clippy::cast_precision_loss)]
            let elapsed = (now - start) as f32 / 1_000.0;

            if let Some(shape) = frame(elapsed) {
                draw(&splash, &image, shape);
            }

            glib::ControlFlow::Continue
        });

        // The timer is what ends the animation, rather than the last frame:
        // it runs whether or not the window was ever drawn, and an overlay
        // left on the screen would sit above everything, swallowing the
        // clicks meant for whatever is under it.
        let playing = Rc::clone(&self.playing);
        let window = self.window.clone();
        let ending = glib::timeout_add_local_once(lifetime(), move || {
            if let Some(Playing { ticking, .. }) = playing.take() {
                ticking.remove();
            }

            window.set_visible(false);
        });

        self.playing.replace(Some(Playing { ticking, ending }));
    }

    /// Takes the window back from an animation, whether or not one is playing.
    fn stop(&self) {
        if let Some(Playing { ticking, ending }) = self.playing.take() {
            ticking.remove();
            ending.remove();
        }
    }

    pub(crate) fn hide(&self) {
        self.unfold();
        self.stop();
        self.window.set_visible(false);
    }
}

/// How big the screen is, for sizes written as a portion of it. The first
/// monitor, which is the only one for most people and a reasonable guess for
/// everyone else.
fn screen() -> (i32, i32) {
    let monitor = gdk::Display::default()
        .and_then(|display| display.monitors().item(0))
        .and_downcast::<gdk::Monitor>();

    monitor.map_or((1920, 1080), |monitor| {
        let geometry = monitor.geometry();

        (geometry.width(), geometry.height())
    })
}

/// What a marker wearing this pair of colours is called.
///
/// Named after the colours themselves rather than numbered, so a class that
/// outlives the sheet it was written for is still the right class, and two
/// windows that happen to look alike share one rule.
fn class(Tint(from, to): Tint) -> String {
    format!(
        "tint-{:02x}{:02x}{:02x}-{:02x}{:02x}{:02x}",
        from.red, from.green, from.blue, to.red, to.green, to.blue
    )
}

/// The gradient itself.
///
/// Two classes deep, so it wins against `.pill` and `.thumbnail` whatever
/// order those are written in. The colours are laid over the panel faintly:
/// the marker's own title sits on top of them and has to stay readable, and a
/// marker still shouldn't read as a window that's showing something.
fn rule(tint: Tint) -> String {
    let Tint(from, to) = tint;
    let colour = |Rgb { red, green, blue }: Rgb| format!("rgba({red},{green},{blue},{TINT})");

    format!(
        ".pill.{} {{ background-color: transparent; \
         background-image: linear-gradient(to right, {}, {}); }}\n",
        class(tint),
        colour(from),
        colour(to),
    )
}

/// Installs the stylesheet, and a second, empty one for the gradients taken
/// from windows.
///
/// The second is registered once and rewritten in place from then on: adding
/// a provider per switch would pile them up for as long as the daemon runs.
fn load_style() -> Result<gtk4::CssProvider> {
    let display = gdk::Display::default().context("no display to draw on")?;
    let style = gtk4::CssProvider::new();
    style.load_from_string(STYLE);

    let tints = gtk4::CssProvider::new();

    for provider in [&style, &tints] {
        gtk4::style_context_add_provider_for_display(
            &display,
            provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    Ok(tints)
}

/// Where the icon is, `elapsed` milliseconds in, or `None` once it is over.
///
/// Two motions, laid over one another. Arriving, it drifts up and grows into
/// itself while it fades in, slowing as it lands. Leaving, it goes on growing
/// — quickly at first and then easing off — while the fade does the opposite,
/// holding for a moment and then taking it all at once. The two pulling
/// against each other is what makes it read as something opening rather than
/// something simply being removed.
#[allow(clippy::cast_possible_truncation)]
fn frame(elapsed: f32) -> Option<Frame> {
    if elapsed >= APPEARING + LEAVING {
        return None;
    }

    // Each saturates at its own end of the animation, so no phase of it needs
    // to be told apart from any other.
    let arrived = eased(elapsed / APPEARING);
    let gone = (elapsed - APPEARING) / LEAVING;
    let above = ((1.0 - arrived) * LIFT) as i32;

    Some(Frame {
        opacity: (FAINT + (1.0 - FAINT) * arrived) * (1.0 - gathering(gone)),
        above,
        below: LIFT as i32 - above,
        size: ((GROW + (1.0 - GROW) * arrived + (GROWN - 1.0) * eased(gone))
            * STARTING_SIZE as f32) as i32,
    })
}

/// Puts one frame of the animation on screen.
fn draw(splash: &gtk4::Box, icon: &gtk4::Image, shape: Frame) {
    splash.set_opacity(f64::from(shape.opacity));
    // The space above and below the icon always adds up to the same, so that
    // it drifts without the face changing size under it.
    icon.set_margin_top(shape.above);
    icon.set_margin_bottom(shape.below);
    icon.set_pixel_size(shape.size);
}

/// The room the icon is drawn in: as big as it ever gets, so that the face it
/// is on is one size from the first frame to the last.
#[allow(clippy::cast_possible_truncation)]
fn biggest() -> i32 {
    (GROWN * STARTING_SIZE as f32) as i32
}

/// Where the icon is at one moment: how solid it is, the space above and below
/// it, and how big it is drawn.
#[derive(Clone, Copy)]
struct Frame {
    opacity: f32,
    above: i32,
    below: i32,
    size: i32,
}

/// One window of a row mid-handover: a marker growing into a thumbnail, or the
/// other way about.
struct Morphing {
    tile: gtk4::Box,
    frame: gtk4::Overlay,
    picture: gtk4::Picture,
    marker: gtk4::Label,
    title: gtk4::Label,
    width: i32,
}

/// A row mid-handover, and what it settles into.
struct Morph {
    sideways: gtk4::ScrolledWindow,
    settled: gtk4::Widget,
    windows: Vec<Morphing>,
    /// How tall the row is when it is open, and how tall one thumbnail is.
    open: i32,
    tall: i32,
    opening: bool,
}

impl Morph {
    /// Puts the row where it is `through` of the way through the handover.
    #[allow(clippy::cast_possible_truncation)]
    fn draw(&self, through: f32) {
        let through = if self.opening { through } else { 1.0 - through };
        let between = |to: i32| PILL + ((to - PILL) as f32 * through) as i32;

        // The row is held at a height of its own, so that what it contains can
        // be taller than it is and be cut off at the fold.
        // Let go of the floor before raising the ceiling: a scroller will not
        // hold a minimum above its own maximum, and the two cross on the way
        // up.
        let height = between(self.open);
        self.sideways.set_min_content_height(-1);
        self.sideways.set_max_content_height(height);
        self.sideways.set_min_content_height(height);

        let frame = between(self.tall);

        for window in &self.windows {
            window.frame.set_size_request(window.width, frame);
            // The thumbnail is stretched to whatever height the marker is at,
            // so it is squashed flat to begin with and whole at the end; the
            // marker's own colours show through it until it is.
            window.picture.set_opacity(f64::from(through));
            window.marker.set_opacity(f64::from(1.0 - through));
            window.title.set_opacity(f64::from(through));
        }
    }

    /// Puts the row back to how it rests, which is built by the same code that
    /// builds every other row.
    fn settle(&self) {
        self.sideways.set_child(Some(&self.settled));
        self.sideways.set_min_content_height(-1);
        self.sideways.set_max_content_height(-1);
        self.sideways
            .set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
        self.sideways.vadjustment().set_value(0.0);
    }
}

/// A handover in progress: the frames it draws, and the timer that ends it.
struct Folding {
    ticking: gtk4::TickCallbackId,
    ending: glib::SourceId,
}

/// One window of a row that is folding or opening.
///
/// A marker and a thumbnail are the same width, so the only thing that changes
/// is the height — and the thumbnail is laid over the marker rather than
/// beside it, so that it arrives out of the marker's own colours.
fn morphing(title: &str, shape: f32, tall: u32, texture: Option<&gdk::MemoryTexture>) -> Morphing {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = ((tall as f32 * shape) as i32).max(4);

    let marker = gtk4::Label::new(Some(title));
    marker.add_css_class("marker");
    marker.set_ellipsize(pango::EllipsizeMode::End);
    marker.set_xalign(0.0);
    marker.set_hexpand(true);
    marker.set_max_width_chars(1);
    marker.set_valign(gtk4::Align::Center);

    let picture = gtk4::Picture::new();
    // Stretched rather than fitted: it covers the marker at every height it
    // passes through, which is what makes it look like one thing growing
    // rather than two things crossing over.
    picture.set_content_fit(gtk4::ContentFit::Fill);

    if let Some(texture) = texture {
        picture.set_paintable(Some(texture));
    }

    let frame = gtk4::Overlay::new();
    frame.add_css_class("thumbnail");
    frame.add_css_class("pill");
    frame.set_valign(gtk4::Align::Center);
    // Explicitly, so that the name inside can fill the frame without the frame
    // itself filling the row: it is the width of its own window, as a marker
    // and as a thumbnail alike.
    frame.set_hexpand(false);
    frame.set_child(Some(&marker));
    frame.add_overlay(&picture);

    let title = gtk4::Label::new(Some(title));
    title.add_css_class("title");
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_max_width_chars(1);
    title.set_xalign(0.0);

    let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    tile.add_css_class("tile");
    tile.set_hexpand(false);
    tile.append(&frame);
    tile.append(&title);

    Morphing {
        tile,
        frame,
        picture,
        marker,
        title,
        width,
    }
}

/// An animation in progress: the frames it draws, and the timer that ends it.
/// An animation in progress: the frames it draws, and the timer that ends it.
struct Playing {
    ticking: gtk4::TickCallbackId,
    ending: glib::SourceId,
}

/// How long the window stays up for: the animation, and a moment after it for
/// a first frame that arrives late.
fn lifetime() -> Duration {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Duration::from_millis((APPEARING + LEAVING + SLACK) as u64)
}

/// Fast to begin with and slowing as it arrives, which is how things move when
/// they are settling into place rather than being pushed.
fn eased(through: f32) -> f32 {
    let left = 1.0 - through.clamp(0.0, 1.0);

    1.0 - left * left * left
}

/// Quick away and a long settle, which is the shape the compositor moves its
/// own windows and layers on.
fn quintic(through: f32) -> f32 {
    let left = 1.0 - through.clamp(0.0, 1.0);

    1.0 - left * left * left * left * left
}

/// Slow to begin with and gathering pace, which is how things move when they
/// are leaving rather than arriving.
fn gathering(through: f32) -> f32 {
    let through = through.clamp(0.0, 1.0);

    through * through
}

/// Whether the icon theme has anything under this name.
fn has_icon(name: &str) -> bool {
    gdk::Display::default()
        .map(|display| gtk4::IconTheme::for_display(&display))
        .is_some_and(|theme| theme.has_icon(name))
}

/// Lays a label out as one line of the panel: left aligned, taking the room
/// that is left over, and cut short with an ellipsis when there isn't enough.
fn line(label: &gtk4::Label) {
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_ellipsize(pango::EllipsizeMode::End);
    // Enough of a hint for the label to give way to the panel's width rather
    // than the other way round: without it a long window title asks for room
    // for the whole of itself, and the panel widens to give it.
    label.set_max_width_chars(1);
}

/// The marker standing in for a window of an application that isn't the one
/// being switched to.
///
/// It is exactly as wide as that window's thumbnail would have been, so a row
/// of markers has the same rhythm as the row of thumbnails it stands in for —
/// only a line tall instead of a thumbnail tall.
fn pill(title: &str, shape: f32, thumbnail: u32) -> gtk4::Box {
    let pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    // The same frame a thumbnail sits in, so a marker reads as the window it
    // stands for rather than as a blob.
    pill.add_css_class("thumbnail");
    pill.add_css_class("pill");
    pill.set_valign(gtk4::Align::Center);
    // Explicitly, so the name inside can expand to fill the marker without the
    // marker itself expanding to fill the row.
    pill.set_hexpand(false);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (thumbnail as f32 * shape) as i32;
    // Even the narrowest window has to be visible as something.
    pill.set_size_request(width.max(4), PILL);

    let label = gtk4::Label::new(Some(title));
    label.add_css_class("marker");
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_xalign(0.0);
    // The name fills the marker, and asks for no width of its own: a marker
    // that grew to fit its window's title would stop being that window's
    // size, which is the whole point of it.
    label.set_hexpand(true);
    label.set_max_width_chars(1);
    pill.append(&label);

    pill
}

/// An application's name, its icon, and the key that switches to it.
///
/// `width` is how many characters of the name to make room for; without one
/// the name takes only the room it needs, which is what a line of applications
/// side by side wants.
fn group_header(
    name: &str,
    app_id: &str,
    icon: Option<&str>,
    trigger: Option<&str>,
    icons: bool,
    width: Option<i32>,
) -> gtk4::Box {
    let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 7);
    header.set_valign(gtk4::Align::Center);

    if let Some(trigger) = trigger {
        let key = gtk4::Label::new(Some(trigger));
        key.add_css_class("keycap");
        header.append(&key);
    }

    if icons {
        match app_icon(app_id, icon) {
            Some(icon) => header.append(&icon),
            // An application the icon theme has nothing for still takes the
            // room an icon would have, or the names after it would sit a
            // little to the left of everyone else's.
            None => {
                let gap = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                gap.set_size_request(ICON_SIZE, ICON_SIZE);
                header.append(&gap);
            }
        }
    }

    let label = gtk4::Label::new(Some(&name.to_uppercase()));
    label.add_css_class("group");
    label.set_xalign(0.0);
    // Asks for a fixed span of characters and ellipsises past it. Both halves
    // matter: an ellipsised label that only sets a maximum asks for nothing
    // and gets it, which collapses every name in the column to one letter.
    // Only a name in a fixed column ellipsises. One that takes the room it
    // needs must not: an ellipsised label asks for almost nothing, and a line
    // of them would be squeezed to initials rather than cut off at the end.
    if let Some(width) = width {
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_width_chars(width);
        label.set_max_width_chars(width);
    }
    header.append(&label);

    header
}

/// Every desktop entry on the system: what the entry itself is called, the
/// command it runs, and the name it gives the application.
///
/// Read straight off the disk rather than asked of GIO, whose answer comes
/// through D-Bus and can block — which is the last thing wanted on the path
/// that has to put the switcher on screen.
fn desktop_entries() -> Vec<Entry> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let shared =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());

    let roots = home
        .into_iter()
        .chain(shared.split(':').map(PathBuf::from))
        .map(|root| root.join("applications"));

    let mut entries = Vec::new();

    for root in roots {
        let Ok(files) = std::fs::read_dir(root) else {
            continue;
        };

        for file in files.flatten() {
            let path = file.path();

            if path.extension().and_then(|end| end.to_str()) != Some("desktop") {
                continue;
            }

            let (Some(entry), Ok(text)) = (
                path.file_stem().and_then(|stem| stem.to_str()),
                std::fs::read_to_string(&path),
            ) else {
                continue;
            };

            if let Some(mut described) = describes(&text) {
                described.entry = entry.to_lowercase();
                entries.push(described);
            }
        }
    }

    entries
}

/// What a desktop entry says, as far as the switcher cares.
fn describes(text: &str) -> Option<Entry> {
    let mut entry = Entry {
        entry: String::new(),
        runs: String::new(),
        class: String::new(),
        name: String::new(),
        icon: String::new(),
    };
    let mut started = false;

    for line in text.lines() {
        // Entries carry a section per language and per action; only the first
        // one describes the application itself.
        if line.starts_with('[') {
            if started {
                break;
            }

            started = true;
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();

        match key {
            "Name" if entry.name.is_empty() => entry.name = value.to_owned(),
            "Icon" if entry.icon.is_empty() => entry.icon = value.to_owned(),
            "StartupWMClass" if entry.class.is_empty() => entry.class = value.to_lowercase(),
            "Exec" if entry.runs.is_empty() => {
                entry.runs = value
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();
            }
            _ => {}
        }
    }

    (!entry.name.is_empty()).then_some(entry)
}

/// What the icon theme has for an application, under a name its desktop entry
/// or its window class suggests.
fn icon_named(app_id: &str, icon: Option<&str>) -> Option<String> {
    let last = app_id.rsplit('.').next().unwrap_or(app_id);

    // What the desktop entry names first: it knows, where the window class is
    // only a guess that happens to be right most of the time.
    icon.map(str::to_owned)
        .into_iter()
        .chain([
            app_id.to_owned(),
            app_id.to_lowercase(),
            last.to_owned(),
            last.to_lowercase(),
        ])
        .find(|name| has_icon(name))
}

/// The application's own icon, if the icon theme has one under a name the
/// window class suggests.
fn app_icon(app_id: &str, icon: Option<&str>) -> Option<gtk4::Image> {
    let icon = gtk4::Image::from_icon_name(&icon_named(app_id, icon)?);
    icon.add_css_class("app-icon");
    icon.set_pixel_size(ICON_SIZE);

    Some(icon)
}

/// One window: what it looks like, when raisin has been able to see it, and
/// what it is called.
///
/// The room for a thumbnail is made now rather than when the capture arrives,
/// so tiles don't jump about as they turn up.
fn tile(
    title: &str,
    preview: Option<u32>,
    shape: f32,
    app_id: &str,
    icon: Option<&str>,
) -> (gtk4::Box, Option<Thumbnailed>) {
    let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    tile.add_css_class("tile");
    tile.set_focusable(true);

    let picture = preview.map(|height| {
        let picture = gtk4::Picture::new();
        picture.set_content_fit(gtk4::ContentFit::Contain);
        picture.set_halign(gtk4::Align::Center);
        picture.set_valign(gtk4::Align::Center);

        // Every thumbnail is the same height and as wide as its own window,
        // so a row of them reads as the windows themselves rather than as a
        // row of identical boxes.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let width = (height as f32 * shape) as i32;

        // Some windows never come back: a compositor only copies what it is
        // drawing, and a browser showing a heavy page can go a long time
        // without drawing one it isn't showing. The application's own icon
        // says which window the tile is, where an empty frame says only that
        // something is broken.
        let standin = app_icon(app_id, icon).inspect(|icon| {
            icon.set_pixel_size(height as i32 / 2);
            icon.add_css_class("standin");
            icon.set_vexpand(true);
        });

        let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        frame.add_css_class("thumbnail");
        frame.set_size_request(width, height as i32);

        if let Some(standin) = &standin {
            frame.append(standin);
        }

        frame.append(&picture);
        tile.append(&frame);

        // A tall, narrow window would otherwise leave no room for its title.
        tile.set_size_request(width.max(height as i32), -1);

        Thumbnailed { picture, standin }
    });

    let label = gtk4::Label::new(Some(title));
    label.add_css_class("title");
    label.set_ellipsize(pango::EllipsizeMode::End);
    // Enough of a hint for the label to give way to the tile's width rather
    // than the other way round.
    label.set_max_width_chars(1);
    label.set_xalign(0.0);
    tile.append(&label);

    (tile, picture)
}

/// How wide a window is against its height, which is the shape its thumbnail
/// comes back. A window the compositor won't measure falls back to the shape a
/// landscape window usually has, and is corrected once it has been captured.
fn shape(size: Option<(u32, u32)>) -> f32 {
    match size {
        Some((width, height)) if height > 0 => width as f32 / height as f32,
        _ => RATIO,
    }
}

/// What the keys do, and the two hints among them that name the key the switch
/// is on.
fn footer(keys: &config::Keys) -> (gtk4::Box, Cycling) {
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    footer.add_css_class("footer");
    // The window title beside it takes the room that is left, which leaves
    // this at the end of the line and takes the title short when there isn't
    // room for both.
    footer.set_valign(gtk4::Align::Center);

    let keycap = |key: &str| {
        let label = gtk4::Label::new(Some(key));
        label.add_css_class("keycap");
        label
    };
    let hint = |cap: &gtk4::Label, text: &str| {
        let says = gtk4::Label::new(Some(text));
        says.add_css_class("hint");
        says.set_valign(gtk4::Align::Center);

        let hint = gtk4::Box::new(gtk4::Orientation::Horizontal, 5);
        hint.append(cap);
        hint.append(&says);

        hint
    };

    // The application's own key walks its windows, and Shift with it walks
    // them the other way. Which key that is belongs to the switch rather than
    // to the configuration, so both are filled in when there is one.
    //
    // Shift and the key share one keycap rather than wearing one each: they
    // are one thing to press, and drawn as one nothing has to say so.
    let forwards = keycap("");
    let backwards = keycap("");
    let cycling = Cycling {
        hints: vec![hint(&forwards, "next"), hint(&backwards, "previous")],
        forwards,
        backwards,
    };

    for hint in &cycling.hints {
        // There is nothing to name until a switch says which key it is on.
        hint.set_visible(false);
        footer.append(hint);
    }

    footer.append(&hint(&keycap(&keycap_name(&keys.cancel)), "cancel"));

    (footer, cycling)
}

/// The hints that name the key the switch is on.
///
/// They are the only part of the panel that says something about the switch
/// rather than about the configuration, so they are kept to be told what to
/// say each time one begins.
struct Cycling {
    hints: Vec<gtk4::Box>,
    forwards: gtk4::Label,
    backwards: gtk4::Label,
}

impl Cycling {
    /// Names the key the switch is on, or takes the hints off the line for an
    /// application that has none.
    fn on(&self, trigger: Option<&str>) {
        for hint in &self.hints {
            hint.set_visible(trigger.is_some());
        }

        if let Some(trigger) = trigger {
            self.forwards.set_label(trigger);
            self.backwards.set_label(&format!("Shift + {trigger}"));
        }
    }
}

/// Puts a gradient on a marker, taking off whichever one it was wearing.
fn wear(pill: &impl IsA<gtk4::Widget>, tint: Tint) {
    for worn in pill.css_classes() {
        if worn.starts_with("tint-") {
            pill.remove_css_class(&worn);
        }
    }

    pill.add_css_class(&class(tint));
}

/// A key as it reads on a keycap rather than in a configuration file.
fn keycap_name(key: &config::Key) -> String {
    key.to_string().replace("Escape", "Esc")
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::{APPEARING, FAINT, LEAVING, LIFT, STARTING_SIZE, biggest, frame};

    #[test]
    fn the_icon_is_already_part_way_in_when_it_appears() {
        let first = frame(0.0).expect("the animation has a first frame");

        assert!((first.opacity - FAINT).abs() < f32::EPSILON);
        assert_eq!(first.above, LIFT as i32);
        assert!(first.size < STARTING_SIZE);
    }

    #[test]
    fn it_arrives_where_it_settles() {
        let arrived = frame(APPEARING).expect("the icon is still there once it lands");

        assert!((arrived.opacity - 1.0).abs() < f32::EPSILON);
        assert_eq!(arrived.above, 0);
        assert_eq!(arrived.size, STARTING_SIZE);
    }

    /// The window is a layer surface with no anchors, so the compositor moves
    /// it whenever it changes size. The drift and the growth both have to
    /// happen inside the room kept for them, or the animation walks about.
    #[test]
    fn the_face_stays_the_same_size_throughout() {
        let mut was = 0;

        for step in 0..=100u8 {
            let at = f32::from(step) * (APPEARING + LEAVING) / 100.0;
            let Some(shape) = frame(at) else {
                continue;
            };

            assert_eq!(shape.above + shape.below, LIFT as i32, "{at}ms in");
            assert!(shape.size >= was, "{at}ms in: the icon shrank");
            assert!(shape.size <= biggest(), "{at}ms in: the icon outgrew its room");
            assert!((0.0..=1.0).contains(&shape.opacity), "{at}ms in");
            was = shape.size;
        }

        assert!(was > STARTING_SIZE, "the icon stopped growing");
    }

    #[test]
    fn it_fades_out_and_ends() {
        let leaving = frame(APPEARING + LEAVING / 2.0).expect("still fading");

        assert!(leaving.opacity > 0.0 && leaving.opacity < 1.0);
        assert!(leaving.size > STARTING_SIZE, "it stopped growing as it left");
        assert!(frame(APPEARING + LEAVING).is_none());
        assert!(frame(f32::MAX).is_none());
    }
}
