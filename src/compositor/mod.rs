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

#[enum_dispatch]
pub trait Compositor {
    fn name(&self) -> &'static str;
    fn get_windows(&self) -> Result<Vec<Window>>;
    fn focus_window(&self, window: &Window) -> Result<()>;
    fn is_running(&self) -> bool;

    fn launch_application(&self, cmd: &str) -> Result<()> {
        let _ = Command::new(cmd)
            .spawn()
            .with_context(|| format!("failed to launch application '{cmd}'"))?;
        Ok(())
    }
}
