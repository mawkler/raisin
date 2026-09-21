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
    padding: 0 8px 14px 8px;
}

.group {
    color: #78819a;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1px;
    padding: 12px 10px 5px 10px;
}

list, list > row {
    background-color: transparent;
    outline: none;
}

list > row {
    color: #c4cad8;
    font-size: 13px;
    padding: 7px 12px;
    border-radius: 10px;
}

.thumbnail {
    border: 1px solid alpha(#ffffff, 0.10);
    border-radius: 6px;
    background-color: alpha(#000000, 0.25);
}

list > row.selected {
    color: #ffffff;
    background-color: alpha(#6b8cff, 0.22);
    box-shadow: inset 2px 0 0 0 #86a4ff;
}

.footer {
    color: #6e7688;
    font-size: 11px;
    padding: 16px 8px 0 8px;
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
";

pub(crate) struct Overlay {
    window: gtk4::Window,
    heading: gtk4::Label,
    list: gtk4::ListBox,
    selected: RefCell<Option<gtk4::ListBoxRow>>,
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

        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(switcher.max_height);

        let footer = footer(keys);

        let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        panel.add_css_class("panel");
        panel.set_size_request(switcher.width, -1);
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
            list,
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
        self.panel.set_size_request(switcher.width, -1);
        self.scroll.set_max_content_height(switcher.max_height);
        self.previews.set(*previews);
        self.icons.set(switcher.icons);

        // The footer names the keys, so it's rebuilt rather than edited.
        let footer = footer(keys);
        self.panel.remove(&*self.footer.borrow());
        self.panel.append(&footer);
        self.footer.replace(footer);
    }

    /// Lists every open window, grouped, with the switch's application named
    /// at the top.
    ///
    /// Only the windows being switched between get a thumbnail: they're the
    /// ones the user is choosing among, and giving every group one would make
    /// the panel taller than the screen.
    pub(crate) fn fill(&self, session: &Session) {
        self.heading
            .set_text(&format!("Switch to {}", session.label()));

        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        self.selected.replace(None);
        self.thumbnails.borrow_mut().clear();

        let previews = self.previews.get();
        let mut group = "";

        for row in session.rows() {
            match row {
                Row::Group(name) => {
                    group = name;
                    self.list.append(&group_row(name, self.icons.get()));
                }
                // A window without a title is better named by its
                // application than by an empty row.
                Row::Window { window, .. } => {
                    let title = if window.title.is_empty() {
                        &window.app_id
                    } else {
                        &window.title
                    };
                    let previewed = previews.enabled
                        && group == session.group()
                        && !window.identifier.is_empty();

                    let (row, thumbnail) = window_row(title, previewed.then_some(previews.width));

                    if let Some(thumbnail) = thumbnail {
                        self.thumbnails
                            .borrow_mut()
                            .insert(window.identifier.clone(), thumbnail);
                    }

                    self.list.append(&row);
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

    /// Marks the window that a release of Super would focus, scrolling it into
    /// view.
    pub(crate) fn highlight(&self, session: &Session) {
        if let Some(previous) = self.selected.replace(None) {
            previous.remove_css_class("selected");
        }

        let Ok(index) = i32::try_from(session.selected_row()) else {
            return;
        };
        let Some(row) = self.list.row_at_index(index) else {
            return;
        };

        row.add_css_class("selected");
        row.grab_focus();
        self.selected.replace(Some(row));
    }

    pub(crate) fn show(&self) {
        self.window.present();
    }

    pub(crate) fn hide(&self) {
        self.window.set_visible(false);
    }
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

fn group_row(name: &str, icons: bool) -> gtk4::ListBoxRow {
    let label = gtk4::Label::new(Some(&name.to_uppercase()));
    label.add_css_class("group");
    line(&label);

    let contents = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    contents.add_css_class("group-row");

    if let Some(icon) = icons.then(|| app_icon(name)).flatten() {
        contents.append(&icon);
    }

    contents.append(&label);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&contents));
    row.set_focusable(false);
    row.set_activatable(false);

    row
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

/// A window's row, with room for its thumbnail when `preview` says how wide
/// one should be. The room is made now rather than when the capture arrives,
/// so that rows don't jump about as thumbnails turn up.
fn window_row(title: &str, preview: Option<u32>) -> (gtk4::ListBoxRow, Option<gtk4::Picture>) {
    let label = gtk4::Label::new(Some(title));
    line(&label);

    let row = gtk4::ListBoxRow::new();
    row.set_activatable(false);

    let Some(width) = preview else {
        row.set_child(Some(&label));

        return (row, None);
    };

    let picture = gtk4::Picture::new();
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_halign(gtk4::Align::Center);
    picture.set_valign(gtk4::Align::Center);

    // The room for the thumbnail is the box, not the thumbnail itself. It is
    // there before any capture arrives, so rows don't jump about as they turn
    // up, and it keeps every row the same height whatever shape the window is:
    // the picture draws at its own size inside it rather than stretching.
    let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    frame.add_css_class("thumbnail");
    frame.set_size_request(width as i32, (width as f32 / RATIO) as i32);
    frame.set_halign(gtk4::Align::Start);
    frame.set_valign(gtk4::Align::Center);
    frame.append(&picture);

    let contents = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    contents.append(&frame);
    contents.append(&label);
    row.set_child(Some(&contents));

    (row, Some(picture))
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
