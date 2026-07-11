use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::os::unix::net;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::compositor::{ActiveCompositor, Compositor, Window};
use crate::input;
use crate::ipc;
use crate::picker::{self, Picker};

fn flat_row_index(picker: &Picker) -> i32 {
    let mut flat = 0;

    for key in picker.groups.keys() {
        if key == &picker.current_group_name {
            break;
        }
        flat += 1 + picker.groups[key].len();
    }
    flat += 1 + picker.current_window_idx;

    i32::try_from(flat).expect("row index exceeds i32 range")
}

fn cycle_and_select(
    picker: &Rc<RefCell<Picker>>,
    list_box: &gtk4::ListBox,
    direction: picker::Direction,
) {
    let mut picker = picker.borrow_mut();
    picker.cycle_window(direction);
    let flat_idx = flat_row_index(&picker);
    if let Some(row) = list_box.row_at_index(flat_idx) {
        list_box.select_row(Some(&row));
        row.grab_focus();
    }
}

fn populate_list_box(picker: &Picker, list_box: &gtk4::ListBox) {
    while let Some(row) = list_box.first_child() {
        list_box.remove(&row);
    }

    for (app_id, windows) in &picker.groups {
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

fn create_list_box(picker: &Picker) -> gtk4::ListBox {
    let list_box = gtk4::ListBox::new();
    list_box.set_activate_on_single_click(false);
    list_box.set_selection_mode(gtk4::SelectionMode::Single);

    populate_list_box(picker, &list_box);

    let idx = flat_row_index(picker);
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

struct GuiState {
    picker: Rc<RefCell<Picker>>,
    list_box: gtk4::ListBox,
    selected_window: Rc<RefCell<Option<Window>>>,
    listener: net::UnixListener,
    trigger_char: Option<char>,
}

impl GuiState {
    fn setup_key_handlers(
        &self,
        controller: &gtk4::EventControllerKey,
        main_loop: &Rc<gtk4::glib::MainLoop>,
    ) {
        let list_box = self.list_box.clone();
        let picker_for_keys = self.picker.clone();
        let loop_for_esc = main_loop.clone();
        let trigger_char = self.trigger_char;

        controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                loop_for_esc.quit();
                return gtk4::glib::Propagation::Stop;
            }

            if let Some(trigger_char) = trigger_char
                && input::matches_trigger_key(key, trigger_char)
            {
                cycle_and_select(&picker_for_keys, &list_box, picker::Direction::Forward);
                return gtk4::glib::Propagation::Stop;
            }

            gtk4::glib::Propagation::Proceed
        });

        let picker_for_release = self.picker.clone();
        let selected = self.selected_window.clone();
        let loop_for_super = main_loop.clone();

        controller.connect_key_released(move |_, key, _, _| {
            if input::is_super_key(key) {
                let picker = picker_for_release.borrow();
                let window_idx = picker.current_window_idx;
                let Some(window) = picker.current_group_windows().get(window_idx) else {
                    log::error!(
                        "could not find any window with index {window_idx} in current group"
                    );
                    return;
                };

                *selected.borrow_mut() = Some(window.clone());
                loop_for_super.quit();
            }
        });
    }

    fn run_event_loop(&self, window: &gtk4::Window) {
        let main_loop = Rc::new(gtk4::glib::MainLoop::new(None, false));
        let controller = gtk4::EventControllerKey::new();

        self.setup_key_handlers(&controller, &main_loop);

        window.add_controller(controller);
        window.present();

        let picker_for_cmds = self.picker.clone();
        let list_box_for_cmds = self.list_box.clone();
        let listener = self.listener.try_clone().expect("failed to clone listener");

        let _ = gtk4::glib::source::timeout_add_local(Duration::from_millis(50), move || {
            let (mut stream, _) = match listener.accept() {
                Ok(accepted) => accepted,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    return gtk4::glib::ControlFlow::Continue;
                }
                Err(err) => {
                    log::error!("listener accept failed: {err}");
                    return gtk4::glib::ControlFlow::Break;
                }
            };

            let _ = stream
                .set_nonblocking(true)
                .inspect_err(|err| log::warn!("failed to make socket stream non-blocking: {err}"));

            let line = BufReader::new(&mut stream)
                .lines()
                .next()
                .and_then(Result::ok)
                .unwrap_or_default();

            let direction: picker::Direction = line.trim().into();
            cycle_and_select(&picker_for_cmds, &list_box_for_cmds, direction);

            gtk4::glib::ControlFlow::Continue
        });

        main_loop.run();
    }
}

pub(crate) fn run(
    search_string: &str,
    trigger_key: Option<char>,
    compositor: &ActiveCompositor,
) -> Result<()> {
    if ipc::try_send(picker::Direction::Forward)? {
        return Ok(());
    }
    let listener = ipc::start_listener().context("failed to listen to socket")?;

    let all_windows = compositor.get_windows()?;
    let focused_app_id = all_windows.first().map(|w| w.app_id.to_lowercase());

    let groups = picker::build_groups(all_windows);

    let Some(current_group_name) = picker::group_name_search(&groups, search_string) else {
        compositor.launch_application(search_string)?;
        return Ok(());
    };

    let current_group_name = current_group_name.clone();

    let current_window_idx = picker::initial_window_idx(
        &groups[&current_group_name],
        &current_group_name,
        focused_app_id.as_deref(),
    );

    let picker = Rc::new(RefCell::new(Picker {
        groups,
        current_group_name,
        current_window_idx,
    }));

    let selected_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(None));

    gtk4::init().context("failed to initialize GTK")?;

    let window = create_overlay_window();
    load_css().context("failed to load CSS")?;

    let list_box = create_list_box(&picker.borrow());
    let header_label = create_header_label(&picker.borrow().current_group_name);
    let footer_label = create_footer_label();

    build_layout(&window, &list_box, &header_label, &footer_label);

    let state = GuiState {
        picker,
        list_box,
        selected_window,
        listener,
        trigger_char: trigger_key,
    };

    state.run_event_loop(&window);

    // Release the exclusive keyboard before asking compositor to focus the target window
    window.set_visible(false);
    while gtk4::glib::MainContext::default().iteration(false) {
        // drain pending events to unmap the layer surface
    }

    if let Some(window) = state.selected_window.borrow().as_ref() {
        compositor.focus_window(window)?;
    }

    Ok(())
}
