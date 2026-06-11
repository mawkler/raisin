use anyhow::Context;
use clap::Parser;

use crate::cli;
use crate::compositor::{ActiveCompositor, Compositor, Window};

pub(crate) struct Application;

impl Application {
    pub(crate) fn run(compositor: ActiveCompositor) -> anyhow::Result<()> {
        let args = cli::Args::parse();

        let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();
        let sibling_windows = compositor
            .get_window_group(search_string)
            .context("failed to get window group")?;

        if sibling_windows.is_empty() {
            compositor.launch_application(&args.app)?;
            return Ok(());
        }

        let target_window = Self::get_cycle_window_target(&sibling_windows);
        compositor
            .focus_window(target_window)
            .context("failed to focus window")?;

        Ok(())
    }

    fn get_cycle_window_target(sibling_windows: &[Window]) -> &Window {
        assert!(!sibling_windows.is_empty());

        let index = if sibling_windows.len() >= 2 { 1 } else { 0 };
        &sibling_windows[index]
    }
}
