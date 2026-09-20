//! The switcher's pure logic: how windows are grouped, which one starts out
//! highlighted, and how cycling moves the highlight.
//!
//! Nothing here talks to a compositor or to GTK, so every rule the
//! specification pins down can be exercised by `cargo test`.

use std::collections::BTreeMap;

use crate::compositor::Window;

/// Every open window, keyed by lowercased `app_id`.
///
/// A `BTreeMap` keeps the groups sorted alphabetically, which is the order they
/// are listed in. Windows inside a group keep the order the compositor returned
/// them in: most recently used first.
pub(crate) type Groups = BTreeMap<String, Vec<Window>>;

/// Groups `windows` by their `app_id`, preserving their relative order.
pub(crate) fn group_windows(windows: Vec<Window>) -> Groups {
    windows
        .into_iter()
        .fold(Groups::new(), |mut groups, window| {
            groups
                .entry(window.app_id.to_lowercase())
                .or_default()
                .push(window);
            groups
        })
}

/// Finds the group `search` refers to: an exact `app_id` match if there is one,
/// otherwise the first group whose name contains `search`.
pub(crate) fn find_group<'a>(groups: &'a Groups, search: &str) -> Option<&'a str> {
    let search = search.to_lowercase();

    groups
        .keys()
        .find(|name| *name == &search)
        .or_else(|| groups.keys().find(|name| name.contains(&search)))
        .map(String::as_str)
}

/// Which way through a group the highlight moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

/// One line of the switcher's list.
#[derive(Debug, PartialEq)]
pub(crate) enum Row<'a> {
    /// A group's label.
    Group(&'a str),
    /// A window, and whether it's the one that would be focused right now.
    Window { window: &'a Window, selected: bool },
}

/// An in-progress switch: all open windows, and the one currently highlighted.
#[derive(Debug)]
pub(crate) struct Session {
    groups: Groups,
    group: String,
    index: usize,
    label: String,
}

impl Session {
    /// Starts a session highlighting a window of `group`.
    ///
    /// `focused` is the window the compositor had focused when the session
    /// started, if any. `label` is the application as the user asked for it,
    /// e.g. `brave`, which reads better than the window class does.
    /// `direction` is the way the user asked to go through the group.
    ///
    /// # Panics
    ///
    /// Panics if `group` isn't a non-empty group of `groups`.
    pub(crate) fn new(
        groups: Groups,
        group: &str,
        focused: Option<&Window>,
        label: &str,
        direction: Direction,
    ) -> Self {
        let index = initial_index(&groups[group], focused, direction);

        Self {
            groups,
            group: group.to_owned(),
            index,
            label: label.to_owned(),
        }
    }

    /// The group being switched within, e.g. `"brave-browser"`.
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    /// The application being switched to, as the user named it.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    /// The window that gets focused if the user confirms right now.
    pub(crate) fn selected_window(&self) -> &Window {
        &self.groups[&self.group][self.index]
    }

    /// Moves the highlight one window along the group, wrapping around.
    pub(crate) fn cycle(&mut self, direction: Direction) {
        let windows = &self.groups[&self.group];
        self.index = step(self.index, windows.len(), direction);
    }

    /// Points the session at another application's group, re-running the
    /// pre-selection rule. Cycles instead if it's the group already shown.
    pub(crate) fn switch_to_group(
        &mut self,
        group: &str,
        focused: Option<&Window>,
        label: &str,
        direction: Direction,
    ) {
        if group == self.group {
            self.cycle(direction);
            return;
        }

        self.index = initial_index(&self.groups[group], focused, direction);
        self.group = group.to_owned();
        self.label = label.to_owned();
    }

    /// Looks up the group `search` refers to, among this session's windows.
    pub(crate) fn find_group(&self, search: &str) -> Option<&str> {
        find_group(&self.groups, search)
    }

    /// Every line to display, in order: each group's label followed by its
    /// windows.
    pub(crate) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.groups.iter().flat_map(move |(name, windows)| {
            let group = std::iter::once(Row::Group(name));
            let windows = windows
                .iter()
                .enumerate()
                .map(move |(index, window)| Row::Window {
                    window,
                    selected: name == &self.group && index == self.index,
                });

            group.chain(windows)
        })
    }

    /// The position of the highlighted window among [`Self::rows`], which is
    /// what the list has to scroll to.
    pub(crate) fn selected_row(&self) -> usize {
        let preceding_rows: usize = self
            .groups
            .iter()
            .take_while(|(name, _)| *name != &self.group)
            .map(|(_, windows)| 1 + windows.len())
            .sum();

        preceding_rows + 1 + self.index
    }
}

/// Which window of `windows` starts out highlighted.
///
/// Moving off the focused window takes precedence: if it's one of `windows`,
/// and there's somewhere else to go, its neighbour in `direction` is
/// highlighted. Otherwise going forwards starts at the group's most recently
/// used window and going backwards at its least recently used one, the way
/// Alt-Tab and Alt-Shift-Tab start at opposite ends of the list.
fn initial_index(windows: &[Window], focused: Option<&Window>, direction: Direction) -> usize {
    if windows.len() < 2 {
        return 0;
    }

    let focused_index = focused.and_then(|focused| windows.iter().position(|w| w == focused));

    match (focused_index, direction) {
        (Some(index), direction) => step(index, windows.len(), direction),
        (None, Direction::Forward) => 0,
        (None, Direction::Backward) => windows.len() - 1,
    }
}

/// One step along a list of `length` windows, wrapping at either end.
fn step(index: usize, length: usize, direction: Direction) -> usize {
    match direction {
        Direction::Forward => (index + 1) % length,
        Direction::Backward => (index + length - 1) % length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: &str, app_id: &str, title: &str) -> Window {
        Window {
            id: id.to_owned(),
            app_id: app_id.to_owned(),
            title: title.to_owned(),
        }
    }

    /// Two ghostty windows and one brave window, most recently used first.
    fn windows() -> Vec<Window> {
        vec![
            window("1", "com.mitchellh.ghostty", "ghostty: raisin"),
            window("2", "brave-browser", "Hyprland Wiki"),
            window("3", "com.mitchellh.ghostty", "ghostty: notes"),
        ]
    }

    fn session(focused: Option<&Window>) -> Session {
        let groups = group_windows(windows());
        Session::new(
            groups,
            "com.mitchellh.ghostty",
            focused,
            "ghostty",
            Direction::Forward,
        )
    }

    #[test]
    fn groups_are_alphabetical_and_keep_windows_in_most_recently_used_order() {
        let groups = group_windows(windows());

        assert_eq!(
            groups.keys().collect::<Vec<_>>(),
            ["brave-browser", "com.mitchellh.ghostty"]
        );
        assert_eq!(
            groups["com.mitchellh.ghostty"]
                .iter()
                .map(|w| w.id.as_str())
                .collect::<Vec<_>>(),
            ["1", "3"]
        );
    }

    #[test]
    fn group_is_found_by_exact_app_id_before_substring() {
        let groups = group_windows(vec![
            window("1", "brave-browser", "Hyprland Wiki"),
            window("2", "brave", "Brave, the other one"),
        ]);

        assert_eq!(find_group(&groups, "brave"), Some("brave"));
        assert_eq!(find_group(&groups, "ghostty"), None);
    }

    #[test]
    fn group_is_found_by_substring_and_ignores_case() {
        let groups = group_windows(windows());

        assert_eq!(
            find_group(&groups, "ghostty"),
            Some("com.mitchellh.ghostty")
        );
        assert_eq!(
            find_group(&groups, "GHOSTTY"),
            Some("com.mitchellh.ghostty")
        );
    }

    #[test]
    fn most_recently_used_window_starts_highlighted() {
        let brave = window("2", "brave-browser", "Hyprland Wiki");

        assert_eq!(session(Some(&brave)).selected_window().id, "1");
        assert_eq!(session(None).selected_window().id, "1");
    }

    #[test]
    fn a_focused_window_hands_over_to_the_next_one() {
        let focused = window("1", "com.mitchellh.ghostty", "ghostty: raisin");

        assert_eq!(session(Some(&focused)).selected_window().id, "3");
    }

    #[test]
    fn the_only_window_of_a_group_stays_highlighted() {
        let groups = group_windows(windows());
        let focused = window("2", "brave-browser", "Hyprland Wiki");
        let mut session = Session::new(
            groups,
            "brave-browser",
            Some(&focused),
            "brave",
            Direction::Forward,
        );

        assert_eq!(session.selected_window().id, "2");

        session.cycle(Direction::Forward);

        assert_eq!(session.selected_window().id, "2");
    }

    #[test]
    fn cycling_wraps_from_the_last_window_back_to_the_first() {
        let mut session = session(None);

        assert_eq!(session.selected_window().id, "1");

        session.cycle(Direction::Forward);
        assert_eq!(session.selected_window().id, "3");

        session.cycle(Direction::Forward);
        assert_eq!(session.selected_window().id, "1");
    }

    #[test]
    fn switching_to_another_group_re_runs_the_pre_selection_rule() {
        let focused = window("1", "com.mitchellh.ghostty", "ghostty: raisin");
        let mut session = session(Some(&focused));

        session.switch_to_group("brave-browser", Some(&focused), "brave", Direction::Forward);

        assert_eq!(session.group(), "brave-browser");
        assert_eq!(session.selected_window().id, "2");

        session.switch_to_group(
            "com.mitchellh.ghostty",
            Some(&focused),
            "ghostty",
            Direction::Forward,
        );

        assert_eq!(session.selected_window().id, "3");
    }

    #[test]
    fn switching_to_the_group_already_shown_cycles_instead() {
        let mut session = session(None);

        session.switch_to_group("com.mitchellh.ghostty", None, "ghostty", Direction::Forward);

        assert_eq!(session.selected_window().id, "3");
    }

    #[test]
    fn going_backwards_starts_at_the_least_recently_used_window() {
        let groups = group_windows(windows());
        let session = Session::new(
            groups,
            "com.mitchellh.ghostty",
            None,
            "ghostty",
            Direction::Backward,
        );

        assert_eq!(session.selected_window().id, "3");
    }

    #[test]
    fn going_backwards_from_a_focused_window_picks_the_one_before_it() {
        let focused = window("1", "com.mitchellh.ghostty", "ghostty: raisin");
        let groups = group_windows(windows());
        let session = Session::new(
            groups,
            "com.mitchellh.ghostty",
            Some(&focused),
            "ghostty",
            Direction::Backward,
        );

        assert_eq!(session.selected_window().id, "3");
    }

    #[test]
    fn cycling_backwards_wraps_the_other_way() {
        let mut session = session(None);

        assert_eq!(session.selected_window().id, "1");

        session.cycle(Direction::Backward);
        assert_eq!(session.selected_window().id, "3");

        session.cycle(Direction::Backward);
        assert_eq!(session.selected_window().id, "1");
    }

    #[test]
    fn rows_list_every_window_under_its_group() {
        let session = session(None);
        let rows: Vec<_> = session.rows().collect();

        assert_eq!(
            rows,
            [
                Row::Group("brave-browser"),
                Row::Window {
                    window: &windows()[1],
                    selected: false
                },
                Row::Group("com.mitchellh.ghostty"),
                Row::Window {
                    window: &windows()[0],
                    selected: true
                },
                Row::Window {
                    window: &windows()[2],
                    selected: false
                },
            ]
        );
    }

    #[test]
    fn the_highlighted_row_is_the_one_the_list_scrolls_to() {
        let mut session = session(None);

        assert_eq!(session.selected_row(), 3);

        session.cycle(Direction::Forward);

        assert_eq!(session.selected_row(), 4);
    }
}
