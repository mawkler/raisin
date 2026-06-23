use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{Context, Result};
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::compositor::{ActiveCompositor, Compositor, Window};
use crate::input;
use crate::state::{self, Picker};

fn flat_row_index(state: &Picker) -> i32 {
    let mut flat = 0;

    for key in state.groups.keys() {
        if key == &state.current_group_name {
            break;
        }
        flat += 1 + state.groups[key].len();
    }
    flat += 1 + state.current_window_idx;

    i32::try_from(flat).expect("row index exceeds i32 range")
}

fn populate_list_box(state: &Picker, list_box: &gtk4::ListBox) {
    while let Some(row) = list_box.first_child() {
        list_box.remove(&row);
    }

    for (app_id, windows) in &state.groups {
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

fn create_list_box(state: &Picker) -> gtk4::ListBox {
    let list_box = gtk4::ListBox::new();
    list_box.set_activate_on_single_click(false);
    list_box.set_selection_mode(gtk4::SelectionMode::Single);

    populate_list_box(state, &list_box);

    let idx = flat_row_index(state);
    if let Some(row) = list_box.row_at_index(idx) {
        list_box.select_row(Some(&row));
        row.grab_focus();
    }

    list_box
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
    state: Rc<RefCell<Picker>>,
    selected_window: &Rc<RefCell<Option<Window>>>,
    trigger_char: Option<char>,
) {
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
            && input::matches_trigger_key(key, trigger_char)
        {
            let mut state = state_for_keys.borrow_mut();
            state.advance_window();
            let flat_idx = flat_row_index(&state);
            if let Some(row) = list_box_for_keys.row_at_index(flat_idx) {
                list_box_for_keys.select_row(Some(&row));
                row.grab_focus();
            }
            return gtk4::glib::Propagation::Stop;
        }

        gtk4::glib::Propagation::Proceed
    });

    let selected_for_release = selected_window.clone();
    let loop_for_super = main_loop.clone();

    controller.connect_key_released(move |_ctrl, keyval, _keycode, _state| {
        if input::is_super_key(keyval) {
            let state = state.borrow();
            let window_idx = state.current_window_idx;
            let Some(window) = state.current_group_windows().get(window_idx) else {
                eprintln!("could not find any window with index {window_idx} in current group");
                return;
            };
            *selected_for_release.borrow_mut() = Some(window.clone());
            loop_for_super.quit();
        }
    });

    window.add_controller(controller);
    window.present();

    main_loop.run();
}

pub(crate) fn run(
    search_string: &str,
    trigger_key: Option<char>,
    compositor: &ActiveCompositor,
) -> Result<()> {
    let all_windows = compositor.get_windows()?;
    let focused_app_id = all_windows.first().map(|w| w.app_id.to_lowercase());

    let groups = state::build_groups(all_windows);

    let Some(current_group_name) = state::group_name_search(&groups, search_string) else {
        compositor.launch_application(search_string)?;
        return Ok(());
    };

    let current_group_name = current_group_name.clone();

    let current_window_idx = state::initial_window_idx(
        &groups[&current_group_name],
        &current_group_name,
        focused_app_id.as_deref(),
    );

    let state = Rc::new(RefCell::new(Picker {
        groups,
        current_group_name,
        current_window_idx,
    }));

    let selected_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));

    gtk4::init().context("failed to initialize GTK")?;

    let window = create_overlay_window();
    load_css().context("failed to load CSS")?;

    let list_box = create_list_box(&state.borrow());
    let header_label = create_header_label(&state.borrow().current_group_name);
    let footer_label = create_footer_label();

    build_layout(&window, &list_box, &header_label, &footer_label);

    run_event_loop(&window, &list_box, state, &selected_window, trigger_key);

    if let Some(window) = selected_window.borrow().as_ref() {
        compositor.focus_window(window)?;
    }

    Ok(())
}
