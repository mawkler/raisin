use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::Context;
use anyhow::Result;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::compositor::{ActiveCompositor, Compositor, Window};

type Groups = BTreeMap<String, Vec<Window>>;

struct GuiState {
    groups: Groups,
    current_group_name: String,
    window_idx: usize,
}

impl GuiState {
    fn current_group_windows(&self) -> &[Window] {
        &self.groups[&self.current_group_name]
    }

    fn flat_row_index(&self) -> usize {
        let mut flat = 0;
        for key in self.groups.keys() {
            if key == &self.current_group_name {
                break;
            }
            flat += 1 + self.groups[key].len();
        }
        flat += 1 + self.window_idx;
        flat
    }

    fn populate_list_box(&self, list_box: &gtk4::ListBox) {
        while let Some(row) = list_box.first_child() {
            list_box.remove(&row);
        }

        for (app_id, windows) in &self.groups {
            let header = gtk4::Label::new(Some(app_id));
            header.add_css_class("group-header");
            header.set_halign(gtk4::Align::Start);
            let header_row = gtk4::ListBoxRow::new();
            header_row.set_child(Some(&header));
            header_row.set_selectable(false);
            header_row.set_focusable(false);
            list_box.append(&header_row);

            for window in windows {
                let label = gtk4::Label::new(Some(&window.title));
                label.add_css_class("window-entry");
                label.set_halign(gtk4::Align::Start);
                let entry_row = gtk4::ListBoxRow::new();
                entry_row.set_child(Some(&label));
                entry_row.set_focusable(true);
                list_box.append(&entry_row);
            }
        }
    }

    fn create_list_box(&self) -> gtk4::ListBox {
        let list_box = gtk4::ListBox::new();
        list_box.set_activate_on_single_click(false);
        list_box.set_selection_mode(gtk4::SelectionMode::Single);

        self.populate_list_box(&list_box);

        let idx = self.flat_row_index() as i32;
        if let Some(row) = list_box.row_at_index(idx) {
            list_box.select_row(Some(&row));
            row.grab_focus();
        }

        list_box
    }
}

fn build_groups(windows: Vec<Window>) -> Groups {
    windows
        .into_iter()
        .fold(BTreeMap::new(), |mut acc, window| {
            acc.entry(window.app_id.to_lowercase())
                .or_default()
                .push(window);
            acc
        })
}

fn find_group(groups: &Groups, search_string: &str) -> Option<String> {
    let search = search_string.to_lowercase();
    groups.keys().find(|k| k.contains(&search)).cloned()
}

fn is_super_key(key: gdk::Key) -> bool {
    key == gdk::Key::Super_L || key == gdk::Key::Super_R
}

fn matches_trigger_key(pressed: gdk::Key, trigger_char: char) -> bool {
    let Some(expected_key) = gdk::Key::from_name(&trigger_char.to_string()) else {
        return false;
    };

    let (lower, upper) = expected_key.convert_case();
    pressed == lower || pressed == upper
}

fn initial_window_idx(windows: &[Window], app_id: &str, focused_app_id: Option<&str>) -> usize {
    let is_in_group = focused_app_id == Some(&app_id.to_lowercase());
    if is_in_group && windows.len() >= 2 {
        1
    } else {
        0
    }
}

fn create_overlay_window() -> gtk4::Window {
    let window = gtk4::Window::new();
    window.init_layer_shell();
    window.set_namespace(Some("raisin"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_margin(Edge::Left, 200);
    window.set_margin(Edge::Right, 200);
    window.set_anchor(Edge::Top, true);
    window.set_margin(Edge::Top, 80);
    window.set_default_size(400, 300);
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Bottom, 80);
    window.set_css_classes(&["raisin-window"]);
    window
}

fn load_css() -> Result<()> {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        ".raisin-window { background-color: rgba(35, 35, 35, 0.96); }
         .header { font-size: 18px; font-weight: bold; padding: 8px; color: #ffffff; }
         .footer { font-size: 14px; padding: 8px; color: #aaaaaa; }
         .group-header { font-size: 16px; font-weight: bold; padding: 4px 8px; color: #ffffff; }
         .window-entry { padding: 4px 8px; color: #dddddd; }
         .window-entry:selected { background-color: rgba(80, 120, 220, 0.6); }",
    );
    let display = gdk::Display::default().context("failed to load display")?;
    let priority = gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION;

    gtk4::style_context_add_provider_for_display(&display, &provider, priority);
    Ok(())
}

fn create_header_label(app_id: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(&format!("Switch to {app_id}")));
    label.add_css_class("header");
    label
}

fn create_footer_label() -> gtk4::Label {
    let label = gtk4::Label::new(Some("Release Super to switch \u{00b7} Esc to cancel"));
    label.add_css_class("footer");
    label
}

fn build_layout(
    window: &gtk4::Window,
    list_box: &gtk4::ListBox,
    header_label: &gtk4::Label,
    footer_label: &gtk4::Label,
) {
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_child(Some(list_box));
    scrolled.set_vexpand(true);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.append(header_label);
    vbox.append(&scrolled);
    vbox.append(footer_label);

    window.set_child(Some(&vbox));
}

fn run_event_loop(
    window: &gtk4::Window,
    list_box: &gtk4::ListBox,
    state: Rc<RefCell<GuiState>>,
    selected_window: Rc<RefCell<Option<Window>>>,
    trigger_char: Option<char>,
) -> Result<()> {
    let main_loop = Rc::new(gtk4::glib::MainLoop::new(None, false));
    let controller = gtk4::EventControllerKey::new();

    let list_box_for_keys = list_box.clone();
    let state_for_keys = state.clone();
    let loop_for_esc = main_loop.clone();

    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            loop_for_esc.quit();
            return gtk4::glib::Propagation::Stop;
        }

        if let Some(trigger_char) = trigger_char
            && matches_trigger_key(key, trigger_char)
        {
            let mut state = state_for_keys.borrow_mut();
            let windows = state.current_group_windows();
            if windows.len() >= 2 {
                state.window_idx = (state.window_idx + 1) % windows.len();
                let flat_idx = state.flat_row_index() as i32;
                if let Some(row) = list_box_for_keys.row_at_index(flat_idx) {
                    list_box_for_keys.select_row(Some(&row));
                    row.grab_focus();
                }
            }
            return gtk4::glib::Propagation::Stop;
        }

        gtk4::glib::Propagation::Proceed
    });

    let selected_window = selected_window.clone();
    let loop_for_super = main_loop.clone();

    controller.connect_key_released(move |_ctrl, keyval, _keycode, _state| {
        if is_super_key(keyval) {
            let state = state.borrow();
            let window_idx = state.window_idx;

            let Some(window) = state.current_group_windows().get(window_idx) else {
                eprintln!("could not find any window with index {window_idx} in current group",);
                return;
            };
            *selected_window.borrow_mut() = Some(window.clone());
            loop_for_super.quit();
        }
    });

    window.add_controller(controller);
    window.present();

    main_loop.run();

    Ok(())
}

pub(crate) fn run(
    search_string: &str,
    trigger_key: Option<char>,
    compositor: &ActiveCompositor,
) -> Result<()> {
    let all_windows = compositor.get_windows()?;
    let focused_app_id = all_windows.first().map(|w| w.app_id.to_lowercase());

    let groups = build_groups(all_windows);

    let Some(current_group_name) = find_group(&groups, search_string) else {
        compositor.launch_application(search_string)?;
        return Ok(());
    };

    let window_idx = initial_window_idx(
        &groups[&current_group_name],
        &current_group_name,
        focused_app_id.as_deref(),
    );

    let state = Rc::new(RefCell::new(GuiState {
        groups,
        current_group_name,
        window_idx,
    }));

    let selected_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));

    gtk4::init().context("failed to initialize GTK")?;

    let window = create_overlay_window();
    load_css().context("failed to load CSS")?;

    let list_box = state.borrow().create_list_box();
    let header_label = create_header_label(&state.borrow().current_group_name);
    let footer_label = create_footer_label();

    build_layout(&window, &list_box, &header_label, &footer_label);

    run_event_loop(
        &window,
        &list_box,
        state,
        selected_window.clone(),
        trigger_key,
    );

    if let Some(window) = selected_window.borrow().as_ref() {
        compositor.focus_window(window)?;
    }

    Ok(())
}
