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

    fn get_window_group(&self, search_string: String) -> Result<Vec<Window>> {
        let windows = self.get_windows().context("failed to get windows")?;

        let target_app_id = windows
            .iter()
            .find(|window| window.app_id.to_lowercase().contains(&search_string))
            .map(|window| window.app_id.to_lowercase());

        let Some(target_app_id) = target_app_id else {
            return Ok(vec![]);
        };

        Ok(windows
            .into_iter()
            .filter(|window| window.app_id.to_lowercase() == target_app_id)
            .collect())
    }
}
