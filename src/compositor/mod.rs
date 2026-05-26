use anyhow::{Context, Result};
use std::fmt::Debug;
use std::process::Command;

pub(crate) mod detection;
pub(crate) mod integrations;

/// NOTE: When implementing `Eq` for your `Window` type, do NOT simply derive it
/// as that would compare all fields (including timestamps, titles, etc.).
/// Instead, implement `PartialEq`/`Eq` manually to compare only the window
/// identity (e.g., the window ID field).
pub trait Window: Debug + Send + Sync + Eq {
    /// Last focused timestamp.
    /// Requires `Ord` to sort windows by last-focused time.
    type Timestamp: Ord + Clone + Debug + Send + Sync;

    fn app_id(&self) -> &str;
    fn last_focused(&self) -> &Self::Timestamp;
    #[allow(unused)]
    fn title(&self) -> &str;
}

pub trait Compositor {
    type Win: Window;

    fn get_windows() -> Result<Vec<Self::Win>>;
    fn get_focused_window() -> Result<Option<Self::Win>>;
    fn focus_window(window: &Self::Win) -> Result<()>;
    /// Check if this compositor is currently running. Used for auto-detection at runtime.
    fn is_running() -> bool;

    /// Launch an application by its command name.
    fn launch_application(cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }

    /// Get all windows that have `search_string` as part of their app id, sorted from most to least
    /// recently focused
    fn get_window_group(search_string: String) -> anyhow::Result<Vec<Self::Win>> {
        let windows = Self::get_windows().context("failed to get windows")?;

        let target_app_id = windows
            .iter()
            .find(|window| window.app_id().to_lowercase().contains(&search_string))
            .map(|window| window.app_id().to_lowercase());

        let Some(target_app_id) = target_app_id else {
            return Ok(vec![]);
        };

        let mut matching_windows: Vec<_> = windows
            .into_iter()
            // TODO: add option to do strict matching
            .filter(|window| window.app_id().to_lowercase() == target_app_id)
            .collect();

        matching_windows.sort_by_key(|w| std::cmp::Reverse(w.last_focused().clone()));

        Ok(matching_windows)
    }
}
