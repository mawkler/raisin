use std::collections::BTreeMap;

use crate::compositor::Window;

pub type Groups = BTreeMap<String, Vec<Window>>;

#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Forward,
    Backward,
}

pub struct Picker {
    pub groups: Groups,
    pub current_group_name: String,
    pub current_window_idx: usize,
}

impl Picker {
    #[must_use]
    pub fn current_group_windows(&self) -> &[Window] {
        &self.groups[&self.current_group_name]
    }

    pub fn cycle_window(&mut self, direction: Direction) {
        let windows = self.current_group_windows();
        if windows.len() < 2 {
            return;
        }

        match direction {
            Direction::Forward => {
                self.current_window_idx = (self.current_window_idx + 1) % windows.len();
            }
            Direction::Backward => {
                self.current_window_idx = if self.current_window_idx >= 1 {
                    self.current_window_idx - 1
                } else {
                    windows.len() - 1
                };
            }
        }
    }
}

#[must_use]
pub fn build_groups(windows: Vec<Window>) -> Groups {
    windows.into_iter().fold(Groups::new(), |mut acc, window| {
        acc.entry(window.app_id.to_lowercase())
            .or_default()
            .push(window);
        acc
    })
}

#[must_use]
pub fn group_name_search<'a>(groups: &'a Groups, search_string: &str) -> Option<&'a String> {
    let search = search_string.to_lowercase();

    // Exact match
    let group_name = groups.keys().find(|&name| name == &search);
    if group_name.is_some() {
        return group_name;
    }

    // Substring match
    groups.keys().find(|name| name.contains(&search))
}

#[must_use]
pub fn initial_window_idx(windows: &[Window], app_id: &str, focused_app_id: Option<&str>) -> usize {
    let is_in_group = focused_app_id == Some(&app_id.to_lowercase());
    #[allow(clippy::bool_to_int_with_if)]
    if is_in_group && windows.len() >= 2 {
        1
    } else {
        0
    }
}
