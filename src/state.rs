use std::collections::BTreeMap;

use crate::compositor::Window;

pub(crate) type Groups = BTreeMap<String, Vec<Window>>;

pub(crate) struct Picker {
    pub(crate) groups: Groups,
    pub(crate) current_group_name: String,
    pub(crate) current_window_idx: usize,
}

impl Picker {
    pub(crate) fn current_group_windows(&self) -> &[Window] {
        &self.groups[&self.current_group_name]
    }

    pub(crate) fn advance_window(&mut self) {
        let windows = self.current_group_windows();
        if windows.len() >= 2 {
            self.current_window_idx = (self.current_window_idx + 1) % windows.len();
        }
    }
}

pub(crate) fn build_groups(windows: Vec<Window>) -> Groups {
    windows.into_iter().fold(Groups::new(), |mut acc, window| {
        acc.entry(window.app_id.to_lowercase())
            .or_default()
            .push(window);
        acc
    })
}

pub(crate) fn group_name_search<'a>(groups: &'a Groups, search_string: &str) -> Option<&'a String> {
    let search = search_string.to_lowercase();

    // Exact match
    let group_name = groups.keys().find(|&name| name == &search);
    if group_name.is_some() {
        return group_name;
    }

    // Substring match
    groups.keys().find(|name| name.contains(&search))
}

pub(crate) fn initial_window_idx(
    windows: &[Window],
    app_id: &str,
    focused_app_id: Option<&str>,
) -> usize {
    let is_in_group = focused_app_id == Some(&app_id.to_lowercase());
    if is_in_group && windows.len() >= 2 {
        1
    } else {
        0
    }
}
