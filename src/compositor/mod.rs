use anyhow::{Context, Result};
use enum_dispatch::enum_dispatch;
use std::process::Command;

pub(crate) mod detection;
pub(crate) use detection::{ActiveCompositor, detect};
pub(crate) mod integrations;

#[derive(Debug, Clone)]
pub(crate) struct Window {
    pub id: String,
    pub app_id: String,
    #[allow(dead_code)]
    pub title: String,
}

impl PartialEq for Window {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[enum_dispatch]
pub trait Compositor {
    fn name(&self) -> &'static str;
    /// Gets all windows sorted from most to least recently focused.
    fn get_windows(&self) -> Result<Vec<Window>>;
    /// Gets the currently focused window, if any, otherwise `None`
    fn get_focused_window(&self) -> Result<Option<Window>>;
    /// Switches to `window`
    fn focus_window(&self, window: &Window) -> Result<()>;
    /// Returns `true` if `Self` is currently active
    fn is_running(&self) -> bool;

    /// Runs the command `cmd`
    fn launch_application(&self, cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }
}
