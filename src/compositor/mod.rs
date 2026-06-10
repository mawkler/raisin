use anyhow::{Context, Result};
use std::fmt::Debug;
use std::process::Command;

pub(crate) mod detection;
pub(crate) use detection::{Active, detect};
pub(crate) mod integrations;
pub(crate) use integrations::{hyprland, niri};

pub trait Window: Debug + Send + Sync {
    fn app_id(&self) -> &str;
    #[allow(unused)]
    fn title(&self) -> &str;
}

pub trait Compositor {
    type Win: Window;

    /// Get all open windows sorted from most to least recently focused. I.e. the first returned
    /// window is the most recently focused.
    fn get_windows() -> Result<Vec<Self::Win>>;
    fn focus_window(window: &Self::Win) -> Result<()>;
    fn is_running() -> bool;

    /// Launch an application by its command name.
    fn launch_application(cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }

    /// Get all windows matching `search_string`, keeping the sort order from `get_windows`
    /// (most to least recently focused).
    fn get_window_group(search_string: String) -> anyhow::Result<Vec<Self::Win>> {
        let windows = Self::get_windows().context("failed to get windows")?;

        let target_app_id = windows
            .iter()
            .find(|window| window.app_id().to_lowercase().contains(&search_string))
            .map(|window| window.app_id().to_lowercase());

        let Some(target_app_id) = target_app_id else {
            return Ok(vec![]);
        };

        Ok(windows
            .into_iter()
            // TODO: add option to do strict matching
            .filter(|window| window.app_id().to_lowercase() == target_app_id)
            .collect())
    }
}
