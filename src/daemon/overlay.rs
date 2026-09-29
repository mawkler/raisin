//! The switcher window: a small dark panel, centred, above everything else.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use gtk4::{gdk, glib, pango};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::compositor::Window;
use crate::config;
use crate::preview::{RATIO, Rgb, Thumbnail, Tint};
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

/// How strongly a window's own colours are laid over the panel.
///
/// The marker's title sits on top of them. At this much, even a window that is
/// pure white composites to a background the title still reads against; since
/// white is the worst case, this one number covers every colour and none of
/// them need clamping to stay legible.
const TINT: &str = "0.25";

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

/* An application with nothing open: there to show its key, and no more. */
.absent .group,
.absent .keycap,
.absent .app-icon {
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

.footer {
    padding: 16px 4px 0 4px;
}

/* What each key does. Level with the key beside it, so it needs no padding
   of its own. */
.hint {
    color: #6e7688;
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

/* The keys in the footer say what raisin does rather than name a window, so
   they wear the same grey as the markers rather than the panel's own. */
.hint-key {
    color: #c4cad8;
    border-color: alpha(#ffffff, 0.14);
    background-color: alpha(#8f98ac, 0.20);
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
    scroll: gtk4::ScrolledWindow,
    footer: RefCell<gtk4::Box>,
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

        let title = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        title.add_css_class("title-bar");
        title.append(&heading);
        title.append(&subject);

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

        let footer = footer(keys);

        let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        panel.add_css_class("panel");
        panel.append(&title);
        panel.append(&scroll);
        panel.append(&footer);

        window.set_child(Some(&panel));

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
            strip,
            tiles: RefCell::new(HashMap::new()),
            selected: RefCell::new(None),
            panel,
            scroll,
            footer: RefCell::new(footer),
            thumbnails: RefCell::new(HashMap::new()),
            captures: RefCell::new(HashMap::new()),
            pills: RefCell::new(HashMap::new()),
            tints,
            rules: RefCell::new(BTreeSet::new()),
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
        let footer = footer(keys);
        self.panel.remove(&*self.footer.borrow());
        self.panel.append(&footer);
        self.footer.replace(footer);
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

        while let Some(block) = self.strip.first_child() {
            self.strip.remove(&block);
        }
        self.selected.replace(None);
        self.tiles.borrow_mut().clear();
        self.thumbnails.borrow_mut().clear();
        self.pills.borrow_mut().clear();

        let previews = self.previews.get();

        // Every row's name takes the same width, so the windows all start in
        // the same place however long the applications are called.
        let names = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);

        // Gathered before anything is built, because rows are ordered by the
        // name on screen rather than by the `app_id` they are keyed under:
        // `Ghostty` and `com.mitchellh.ghostty` sort nothing alike.
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

        groups.sort_by_key(|(_, name, _)| name.to_lowercase());

        for (app_id, name, group) in &groups {
            // Only the application being switched to shows its windows in
            // full. Every other window is one marker, the shape of the window
            // it stands for, so the row is a line tall.
            let targeted = *app_id == session.group();
            let windows =
                gtk4::Box::new(gtk4::Orientation::Horizontal, if targeted { 8 } else { 5 });

            for window in group {
                if targeted {
                    self.append_tile(&windows, window, previews);
                } else {
                    let title = if window.title.is_empty() {
                        &window.app_id
                    } else {
                        &window.title
                    };

                    let pill = pill(title, shape(window.size), previews.height);

                    if let Some(tint) = self
                        .captures
                        .borrow()
                        .get(&window.identifier)
                        .and_then(|capture| capture.tint)
                    {
                        self.rule(tint);
                        pill.add_css_class(&class(tint));
                    }

                    windows.append(&pill);
                    self.pills
                        .borrow_mut()
                        .insert(window.identifier.clone(), pill);
                }
            }

            // A row of windows scrolls sideways on its own when there are more
            // of them than the panel is wide.
            let sideways = gtk4::ScrolledWindow::new();
            sideways.set_child(Some(&windows));
            sideways.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
            sideways.set_propagate_natural_height(true);
            sideways.set_hexpand(true);

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
            waiting.sort_by_key(|(name, _)| name.to_lowercase());

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

            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
            row.add_css_class("row");
            row.add_css_class("absent");
            row.append(&cut_off);

            self.strip.append(&row);
        }

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
        let Some(pill) = self.pills.borrow().get(identifier).cloned() else {
            // No marker for it: the colour is still worth keeping, for the
            // next time this window is one.
            self.rule(tint);
            return;
        };

        if let Some(worn) = self
            .captures
            .borrow()
            .get(identifier)
            .and_then(|capture| capture.tint)
        {
            pill.remove_css_class(&class(worn));
        }

        self.rule(tint);
        pill.add_css_class(&class(tint));
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
        self.window.present();
    }

    pub(crate) fn hide(&self) {
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

/// Lays a label out as one line of the panel: left aligned, filling the
/// width, and cut short with an ellipsis rather than shrinking to fit.
fn line(label: &gtk4::Label) {
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_ellipsize(pango::EllipsizeMode::End);
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

/// The application's own icon, if the icon theme has one under a name the
/// window class suggests.
fn app_icon(app_id: &str, icon: Option<&str>) -> Option<gtk4::Image> {
    let theme = gtk4::IconTheme::for_display(&gdk::Display::default()?);
    let last = app_id.rsplit('.').next().unwrap_or(app_id);
    // What the desktop entry names first: it knows, where the window class is
    // only a guess that happens to be right most of the time.
    let candidates: Vec<String> = icon
        .map(str::to_owned)
        .into_iter()
        .chain([
            app_id.to_owned(),
            app_id.to_lowercase(),
            last.to_owned(),
            last.to_lowercase(),
        ])
        .collect();

    let name = candidates.iter().find(|name| theme.has_icon(name))?;
    let icon = gtk4::Image::from_icon_name(name);
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

fn footer(keys: &config::Keys) -> gtk4::Box {
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 7);
    footer.add_css_class("footer");
    // Off to the right, where it stays out of the way of the names running
    // down the left.
    footer.set_halign(gtk4::Align::End);

    let keycap = |key: &str| {
        let label = gtk4::Label::new(Some(key));
        label.add_css_class("keycap");
        label.add_css_class("hint-key");
        label
    };
    let hint = |text: &str| {
        let label = gtk4::Label::new(Some(text));
        // Its own class, not the footer's: the footer's padding is what holds
        // the whole row clear of the strip above it, and on a label it would
        // pad the text itself downwards instead.
        label.add_css_class("hint");
        label.set_valign(gtk4::Align::Center);
        label
    };

    footer.append(&keycap("Super"));
    footer.append(&hint("release to switch"));

    if let Some(next) = &keys.next {
        footer.append(&keycap(&keycap_name(next)));
        footer.append(&hint("next"));
    }

    footer.append(&keycap(&keycap_name(&keys.cancel)));
    footer.append(&hint("cancel"));

    footer
}

/// A key as it reads on a keycap rather than in a configuration file.
fn keycap_name(key: &config::Key) -> String {
    key.to_string().replace("Escape", "Esc")
}
