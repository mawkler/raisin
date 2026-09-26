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
    pub title: String,
    /// What the window is called by the Wayland protocols that capture it,
    /// which is not what the compositor's own IPC calls it. Empty when the
    /// compositor doesn't offer one, which means no preview for this window.
    pub identifier: String,
    /// How big the window is on screen, which is the shape its thumbnail will
    /// come back. `None` when the compositor doesn't say, and the tile falls
    /// back to a common shape until the capture arrives.
    pub size: Option<(u32, u32)>,
    /// What the window called itself when it opened. Applications usually put
    /// their own name there before they have a document to name instead, which
    /// makes it a far better label for a group than the Wayland class.
    pub initial_title: String,
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
