use anyhow::{Context, Result};
use clap::Parser;

use crate::cli;
use crate::compositor::{ActiveCompositor, Compositor, Window};

pub(crate) struct Application {
    compositor: ActiveCompositor,
}

impl Application {
    pub(crate) fn new(compositor: ActiveCompositor) -> Self {
        Self { compositor }
    }

    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let args = cli::Args::parse();

        let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();
        let sibling_windows = self
            .get_window_group(search_string)
            .context("failed to get window group")?;

        if sibling_windows.is_empty() {
            self.compositor.launch_application(&args.app)?;
            return Ok(());
        }

        let target_window = Self::get_cycle_window_target(&sibling_windows);
        self.compositor
            .focus_window(target_window)
            .context("failed to focus window")?;

        Ok(())
    }

    fn get_cycle_window_target(sibling_windows: &[Window]) -> &Window {
        assert!(!sibling_windows.is_empty());

        let index = if sibling_windows.len() >= 2 { 1 } else { 0 };
        &sibling_windows[index]
    }

    fn get_window_group(&self, search_string: String) -> Result<Vec<Window>> {
        let windows = self
            .compositor
            .get_windows()
            .context("failed to get windows")?;

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
