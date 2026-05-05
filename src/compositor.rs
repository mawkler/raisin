// TODO: perhaps anyhow shouldn't be used here?
use anyhow::{Context, Result};
use std::fmt::Debug;
use std::process::Command;

pub trait Window: Debug + Send + Sync {
    /// Window identifier (e.g., `u32` for Niri, `String` for others).
    /// Requires `Eq` to compare focused window with matching windows.
    type Id: Eq + Clone + Debug + Send + Sync;

    /// Focus timestamp type (e.g., Niri's secs/nanos struct).
    /// Requires `Ord` to sort windows by last-focused time.
    type Timestamp: Ord + Clone + Debug + Send + Sync;

    fn id(&self) -> &Self::Id;
    fn app_id(&self) -> &str;
    fn last_focused(&self) -> &Self::Timestamp;
    fn title(&self) -> &str;
}

pub trait Compositor {
    type Win: Window;

    fn get_windows() -> Result<Vec<Self::Win>>;
    fn get_focused_window() -> Result<Option<Self::Win>>;
    fn focus_window(id: &<Self::Win as Window>::Id) -> Result<()>;

    /// Launch an application by its command name.
    fn launch_application(cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }
}
