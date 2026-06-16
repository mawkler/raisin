use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::compositor::{ActiveCompositor, Compositor, Window};

struct WindowGroup {
    app_id: String,
    windows: Vec<Window>,
}

struct GuiState {
    groups: Vec<WindowGroup>,
    group_idx: usize,
    window_idx: usize,
}

fn build_groups(windows: Vec<Window>) -> Vec<WindowGroup> {
    let mut groups: Vec<WindowGroup> = Vec::new();
    for window in windows {
        let app_id = window.app_id.clone();
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.app_id.eq_ignore_ascii_case(&app_id))
        {
            group.windows.push(window);
        } else {
            groups.push(WindowGroup {
                app_id,
                windows: vec![window],
            });
        }
    }
    groups
}

fn find_group(groups: &[WindowGroup], search_string: &str) -> Option<usize> {
    let search = search_string.to_lowercase();
    groups
        .iter()
        .position(|g| g.app_id.to_lowercase().contains(&search))
}

fn flat_row_index(state: &GuiState) -> usize {
    let mut flat = 0;
    for i in 0..state.group_idx {
        flat += 1 + state.groups[i].windows.len();
    }
    flat += 1 + state.window_idx;
    flat
}

fn build_window_list(list_box: &gtk4::ListBox, state: &GuiState) {
    while let Some(row) = list_box.first_child() {
        list_box.remove(&row);
    }

    for group in &state.groups {
        let header = gtk4::Label::new(Some(&group.app_id));
        header.add_css_class("group-header");
        header.set_halign(gtk4::Align::Start);
        let header_row = gtk4::ListBoxRow::new();
        header_row.set_child(Some(&header));
        header_row.set_selectable(false);
        header_row.set_focusable(false);
        list_box.append(&header_row);

        for window in &group.windows {
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

fn is_super_key(keyval: gdk::Key) -> bool {
    keyval == gdk::Key::Super_L || keyval == gdk::Key::Super_R
}

fn matches_trigger_key(pressed: gdk::Key, trigger_char: char) -> bool {
    let name = trigger_char.to_string();
    let Some(expected) = gdk::Key::from_name(&name) else {
        return false;
    };
    let (lower, upper) = expected.convert_case();
    pressed == lower || pressed == upper
}

pub(crate) fn run(
    search_string: &str,
    trigger_key: Option<&str>,
    compositor: &ActiveCompositor,
) -> Result<()> {
    let all_windows = compositor.get_windows()?;

    let focused_app_id = all_windows.first().map(|w| w.app_id.to_lowercase());

    let groups = build_groups(all_windows);

    let Some(group_idx) = find_group(&groups, search_string) else {
        compositor.launch_application(search_string)?;
        return Ok(());
    };

    let current_is_in_group =
        focused_app_id.as_deref() == Some(&groups[group_idx].app_id.to_lowercase());

    let initial_window_idx = if current_is_in_group && groups[group_idx].windows.len() >= 2 {
        1
    } else {
        0
    };

    let state = Rc::new(RefCell::new(GuiState {
        groups,
        group_idx,
        window_idx: initial_window_idx,
    }));

    let selected_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));

    let trigger_char = trigger_key.and_then(|k| k.chars().next());

    gtk4::init()?;

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

    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        ".raisin-window { background-color: rgba(35, 35, 35, 0.96); }
         .header { font-size: 18px; font-weight: bold; padding: 8px; color: #ffffff; }
         .footer { font-size: 14px; padding: 8px; color: #aaaaaa; }
         .group-header { font-size: 16px; font-weight: bold; padding: 4px 8px; color: #ffffff; }
         .window-entry { padding: 4px 8px; color: #dddddd; }
         .window-entry:selected { background-color: rgba(80, 120, 220, 0.6); }",
    );
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let list_box = gtk4::ListBox::new();
    list_box.set_activate_on_single_click(false);
    list_box.set_selection_mode(gtk4::SelectionMode::Single);

    {
        let state = state.borrow();
        build_window_list(&list_box, &state);

        let idx = flat_row_index(&state) as i32;
        if let Some(row) = list_box.row_at_index(idx) {
            list_box.select_row(Some(&row));
            row.grab_focus();
        }
    }

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_child(Some(&list_box));
    scrolled.set_vexpand(true);

    let header_label = {
        let state = state.borrow();
        let current_app = &state.groups[state.group_idx].app_id;
        let label = gtk4::Label::new(Some(&format!("Switch to {current_app}")));
        label.add_css_class("header");
        label
    };

    let footer_label = {
        let label = gtk4::Label::new(Some("Release Super to switch \u{00b7} Esc to cancel"));
        label.add_css_class("footer");
        label
    };

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.append(&header_label);
    vbox.append(&scrolled);
    vbox.append(&footer_label);

    window.set_child(Some(&vbox));

    let loop_ = Rc::new(gtk4::glib::MainLoop::new(None, false));

    let controller = gtk4::EventControllerKey::new();
    let list_box_for_keys = list_box.clone();
    let state_for_keys = state.clone();
    let selected_for_release = selected_window.clone();
    let loop_for_esc = loop_.clone();
    let loop_for_super = loop_.clone();

    controller.connect_key_pressed(move |_ctrl, keyval, _keycode, _state| {
        if keyval == gdk::Key::Escape {
            loop_for_esc.quit();
            return gtk4::glib::Propagation::Stop;
        }

        if let Some(trigger_char) = trigger_char
            && matches_trigger_key(keyval, trigger_char)
        {
            let mut state = state_for_keys.borrow_mut();
            let group = &state.groups[state.group_idx];
            if group.windows.len() >= 2 {
                state.window_idx = (state.window_idx + 1) % group.windows.len();
                let flat_idx = flat_row_index(&state) as i32;
                if let Some(row) = list_box_for_keys.row_at_index(flat_idx) {
                    list_box_for_keys.select_row(Some(&row));
                    row.grab_focus();
                }
            }
            return gtk4::glib::Propagation::Stop;
        }

        gtk4::glib::Propagation::Proceed
    });

    controller.connect_key_released(move |_ctrl, keyval, _keycode, _state| {
        if is_super_key(keyval) {
            let state = state.borrow();
            if let Some(window) = state.groups[state.group_idx].windows.get(state.window_idx) {
                *selected_for_release.borrow_mut() = Some(window.clone());
            }
            loop_for_super.quit();
        }
    });

    window.add_controller(controller);
    window.present();

    loop_.run();

    if let Some(window) = selected_window.borrow().as_ref() {
        compositor.focus_window(window)?;
    }

    Ok(())
}
