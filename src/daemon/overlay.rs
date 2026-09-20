//! The switcher window: a small dark panel, centred, above everything else.

use std::cell::RefCell;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use gtk4::{gdk, pango};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::config;
use crate::switcher::{Row, Session};

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
}

impl Overlay {
    /// Builds the window, ready to be put on screen later.
    pub(crate) fn new(switcher: &config::Switcher, keys: &config::Keys) -> Result<Self> {
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
        })
    }

    /// Takes on a configuration that changed while the daemon was running.
    pub(crate) fn reconfigure(&self, switcher: &config::Switcher, keys: &config::Keys) {
        self.panel.set_size_request(switcher.width, -1);
        self.scroll.set_max_content_height(switcher.max_height);

        // The footer names the keys, so it's rebuilt rather than edited.
        let footer = footer(keys);
        self.panel.remove(&*self.footer.borrow());
        self.panel.append(&footer);
        self.footer.replace(footer);
    }

    /// Lists every open window, grouped, with the switch's application named
    /// at the top.
    pub(crate) fn fill(&self, session: &Session) {
        self.heading
            .set_text(&format!("Switch to {}", session.label()));

        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        self.selected.replace(None);

        for row in session.rows() {
            self.list.append(&match row {
                Row::Group(name) => group_row(name),
                // A window without a title is better named by its
                // application than by an empty row.
                Row::Window { window, .. } => window_row(if window.title.is_empty() {
                    &window.app_id
                } else {
                    &window.title
                }),
            });
        }
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

fn group_row(name: &str) -> gtk4::ListBoxRow {
    let label = gtk4::Label::new(Some(&name.to_uppercase()));
    label.add_css_class("group");
    line(&label);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&label));
    row.set_focusable(false);
    row.set_activatable(false);

    row
}

fn window_row(title: &str) -> gtk4::ListBoxRow {
    let label = gtk4::Label::new(Some(title));
    line(&label);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&label));
    row.set_activatable(false);

    row
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
