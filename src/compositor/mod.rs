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
    fn is_running() -> bool {
        false
    }

    /// Launch an application by its command name.
    fn launch_application(cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }
}
