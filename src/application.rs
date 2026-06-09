use std::marker::PhantomData;

use anyhow::Context;
use clap::Parser;

use crate::{cli, compositor::Compositor};

pub(crate) struct Application<C: Compositor>(PhantomData<C>);

impl<C: Compositor> Application<C> {
    pub(crate) fn run() -> anyhow::Result<()> {
        let args = cli::Args::parse();

        let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();
        let sibling_windows =
            C::get_window_group(search_string).context("failed to get window group")?;

        if sibling_windows.is_empty() {
            C::launch_application(&args.app)?;
            return Ok(());
        }

        let target_window = Self::get_cycle_window_target(&sibling_windows);
        C::focus_window(target_window).context("failed to focus window")?;

        Ok(())
    }

    fn get_cycle_window_target(sibling_windows: &[C::Win]) -> &C::Win {
        assert!(!sibling_windows.is_empty());

        let index = if sibling_windows.len() >= 2 { 1 } else { 0 };
        &sibling_windows[index]
    }
}
