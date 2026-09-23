//! The switcher window: a small dark panel, centred, above everything else.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use gtk4::{gdk, glib, pango};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::config;
use crate::preview::{RATIO, Thumbnail};
use crate::switcher::{Row, Session};

/// How big an application's icon is beside its name.
const ICON_SIZE: i32 = 16;

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

.heading {
    color: #eef1f7;
    font-size: 15px;
    font-weight: 600;
    padding: 0 4px 14px 4px;
}

.group {
    color: #78819a;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1px;
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

.footer {
    color: #6e7688;
    font-size: 11px;
    padding: 16px 4px 0 4px;
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

pub(crate) struct Overlay {
    window: gtk4::Window,
    heading: gtk4::Label,
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
    thumbnails: RefCell<HashMap<String, gtk4::Picture>>,
    previews: Cell<config::Previews>,
    icons: Cell<bool>,
}

impl Overlay {
    /// Builds the window, ready to be put on screen later.
    pub(crate) fn new(
        switcher: &config::Switcher,
        keys: &config::Keys,
        previews: &config::Previews,
    ) -> Result<Self> {
        load_style().context("failed to load the switcher's stylesheet")?;

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
        line(&heading);

        let strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 20);

        // The applications run off to the side rather than down the screen, so
        // the panel stays the height of one row of windows however many there
        // are.
        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_child(Some(&strip));
        scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
        scroll.set_propagate_natural_width(true);
        scroll.set_propagate_natural_height(true);
        let (screen_width, screen_height) = screen();
        scroll.set_max_content_width(switcher.width.pixels(screen_width));
        scroll.set_max_content_height(switcher.max_height.pixels(screen_height));

        let footer = footer(keys);

        let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        panel.add_css_class("panel");
        panel.append(&heading);
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
            strip,
            tiles: RefCell::new(HashMap::new()),
            selected: RefCell::new(None),
            panel,
            scroll,
            footer: RefCell::new(footer),
            thumbnails: RefCell::new(HashMap::new()),
            previews: Cell::new(*previews),
            icons: Cell::new(switcher.icons),
        })
    }

    /// Takes on a configuration that changed while the daemon was running.
    pub(crate) fn reconfigure(
        &self,
        switcher: &config::Switcher,
        keys: &config::Keys,
        previews: &config::Previews,
    ) {
        let (screen_width, screen_height) = screen();
        self.scroll
            .set_max_content_width(switcher.width.pixels(screen_width));
        self.scroll
            .set_max_content_height(switcher.max_height.pixels(screen_height));
        self.previews.set(*previews);
        self.icons.set(switcher.icons);

        // The footer names the keys, so it's rebuilt rather than edited.
        let footer = footer(keys);
        self.panel.remove(&*self.footer.borrow());
        self.panel.append(&footer);
        self.footer.replace(footer);
    }

    /// Lays out every open window, its application's key beside the
    /// application's name, with the switch's own application named at the top.
    ///
    /// Only the windows being switched between get a thumbnail: they are the
    /// ones the user is choosing among.
    pub(crate) fn fill(&self, session: &Session, triggers: &HashMap<String, String>) {
        self.heading
            .set_text(&format!("Switch to {}", session.label()));

        while let Some(block) = self.strip.first_child() {
            self.strip.remove(&block);
        }
        self.selected.replace(None);
        self.tiles.borrow_mut().clear();
        self.thumbnails.borrow_mut().clear();

        let previews = self.previews.get();
        let mut windows = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let mut previewed = false;

        for row in session.rows() {
            match row {
                Row::Group(name) => {
                    previewed = previews.enabled && name == session.group();
                    windows = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

                    let block = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
                    block.append(&group_header(
                        name,
                        triggers.get(name).map(String::as_str),
                        self.icons.get(),
                    ));
                    block.append(&windows);

                    self.strip.append(&block);
                }
                // A window without a title is better named by its application
                // than by an empty tile.
                Row::Window { window, .. } => {
                    let title = if window.title.is_empty() {
                        &window.app_id
                    } else {
                        &window.title
                    };
                    let preview =
                        (previewed && !window.identifier.is_empty()).then_some(previews.width);

                    let (tile, thumbnail) = tile(title, preview, previews.width);

                    if let Some(thumbnail) = thumbnail {
                        self.thumbnails
                            .borrow_mut()
                            .insert(window.identifier.clone(), thumbnail);
                    }

                    windows.append(&tile);
                    self.tiles.borrow_mut().insert(window.id.clone(), tile);
                }
            }
        }
    }

    /// Puts a captured window into the row waiting for it, if that row is
    /// still on screen.
    pub(crate) fn set_thumbnail(&self, thumbnail: &Thumbnail) {
        let Some(picture) = self.thumbnails.borrow().get(&thumbnail.identifier).cloned() else {
            return;
        };

        let pixels = glib::Bytes::from_owned(thumbnail.pixels.clone());
        let texture = gdk::MemoryTexture::new(
            thumbnail.width as i32,
            thumbnail.height as i32,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &pixels,
            thumbnail.width as usize * 4,
        );

        picture.set_paintable(Some(&texture));
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

fn load_style() -> Result<()> {
    let display = gdk::Display::default().context("no display to draw on")?;
    let style = gtk4::CssProvider::new();
    style.load_from_string(STYLE);

    gtk4::style_context_add_provider_for_display(
        &display,
        &style,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    Ok(())
}

/// Lays a label out as one line of the panel: left aligned, filling the
/// width, and cut short with an ellipsis rather than shrinking to fit.
fn line(label: &gtk4::Label) {
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_ellipsize(pango::EllipsizeMode::End);
}

/// An application's name, its icon, and the key that switches to it.
fn group_header(name: &str, trigger: Option<&str>, icons: bool) -> gtk4::Box {
    let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 7);

    if let Some(icon) = icons.then(|| app_icon(name)).flatten() {
        header.append(&icon);
    }

    let label = gtk4::Label::new(Some(&name.to_uppercase()));
    label.add_css_class("group");
    // The name takes what the tiles beneath it leave, and gives way before
    // the key does: an ellipsised label would otherwise ask for nothing and
    // get it.
    line(&label);
    header.append(&label);

    if let Some(trigger) = trigger {
        let key = gtk4::Label::new(Some(trigger));
        key.add_css_class("keycap");
        header.append(&key);
    }

    header
}

/// The application's own icon, if the icon theme has one under a name the
/// window class suggests.
fn app_icon(app_id: &str) -> Option<gtk4::Image> {
    let theme = gtk4::IconTheme::for_display(&gdk::Display::default()?);
    let last = app_id.rsplit('.').next().unwrap_or(app_id);
    let candidates = [
        app_id.to_owned(),
        app_id.to_lowercase(),
        last.to_owned(),
        last.to_lowercase(),
    ];

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
fn tile(title: &str, preview: Option<u32>, width: u32) -> (gtk4::Box, Option<gtk4::Picture>) {
    let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    tile.add_css_class("tile");
    tile.set_focusable(true);
    tile.set_size_request(width as i32, -1);

    let picture = preview.map(|width| {
        let picture = gtk4::Picture::new();
        picture.set_content_fit(gtk4::ContentFit::Contain);
        picture.set_halign(gtk4::Align::Center);
        picture.set_valign(gtk4::Align::Center);

        let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        frame.add_css_class("thumbnail");
        frame.set_size_request(width as i32, (width as f32 / RATIO) as i32);
        frame.append(&picture);
        tile.append(&frame);

        picture
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

fn footer(keys: &config::Keys) -> gtk4::Box {
    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 7);
    footer.add_css_class("footer");

    let keycap = |key: &str| {
        let label = gtk4::Label::new(Some(key));
        label.add_css_class("keycap");
        label
    };
    let hint = |text: &str| {
        let label = gtk4::Label::new(Some(text));
        label.add_css_class("footer");
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
