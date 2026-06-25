use anyhow::{Context, Result};

use crate::compositor::{ActiveCompositor, Compositor, Window};
use crate::{cli, gui};

pub(crate) struct Application {
    cli_arguments: cli::Args,
    compositor: ActiveCompositor,
}

impl Application {
    pub(crate) fn new(cli_arguments: cli::Args, compositor: ActiveCompositor) -> Self {
        Self {
            cli_arguments,
            compositor,
        }
    }

    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let args = &self.cli_arguments;

        let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();

        if args.gui {
            let trigger_key = self.cli_arguments.trigger_key;
            return gui::run(&search_string, trigger_key, &self.compositor);
        }

        let sibling_windows = self
            .get_window_group(&search_string)
            .context("failed to get window group")?;

        if sibling_windows.is_empty() {
            self.compositor.launch_application(&args.app)?;
            return Ok(());
        }

        let focused_window = self
            .compositor
            .get_focused_window()
            .context("failed to get focused window")?;

        let target_window =
            Self::get_cycle_window_target(focused_window.as_ref(), &sibling_windows);

        let Window { id, app_id, .. } = target_window;
        log::info!("focusing window {id} ({app_id})");

        self.compositor
            .focus_window(target_window)
            .context("failed to focus window")?;

        Ok(())
    }

    fn get_cycle_window_target<'a>(
        focused_window: Option<&'a Window>,
        sibling_windows: &'a [Window],
    ) -> &'a Window {
        assert!(!sibling_windows.is_empty());

        let target_index = Self::get_cycle_window_target_index(focused_window, sibling_windows);
        &sibling_windows[target_index]
    }

    fn get_cycle_window_target_index(
        focused_window: Option<&Window>,
        sibling_windows: &[Window],
    ) -> usize {
        let Some(focused_window) = focused_window else {
            return 0;
        };

        let focused_app_id = focused_window.app_id.to_lowercase();
        let target_app_id = sibling_windows[0].app_id.to_lowercase();

        // Switching from one application to another
        if focused_app_id != target_app_id {
            return 0;
        }

        let Some(window_position) = sibling_windows.iter().position(|w| w == focused_window) else {
            return 0;
        };

        (window_position + 1) % sibling_windows.len()
    }

    fn search_for_app_id(search_string: &str, windows: &[Window]) -> Option<String> {
        // Exact match
        let target_app_id = windows
            .iter()
            .map(|window| window.app_id.to_lowercase())
            .find(|app_id| app_id == search_string);

        if target_app_id.is_some() {
            return target_app_id;
        }

        // Substring match
        windows
            .iter()
            .map(|window| window.app_id.to_lowercase())
            .find(|app_id| app_id.contains(search_string))
    }

    fn get_window_group(&self, search_string: &str) -> Result<Vec<Window>> {
        let windows = self
            .compositor
            .get_windows()
            .context("failed to get windows")?;

        let Some(target_app_id) = Self::search_for_app_id(search_string, &windows) else {
            return Ok(vec![]);
        };

        Ok(windows
            .into_iter()
            .filter(|window| window.app_id.to_lowercase() == target_app_id)
            .collect())
    }
}
