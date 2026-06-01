use anyhow::Context;
use clap::Parser;
use compositor::Compositor;
use compositor::detection::CompositorInstance;

use crate::compositor::{
    detection::detect_compositor,
    integrations::{hyprland::HyprlandCompositor, niri::NiriCompositor},
};

mod cli;
mod compositor;

fn run<C: Compositor>() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();
    let sibling_windows =
        C::get_window_group(search_string).context("failed to get window group")?;

    if sibling_windows.is_empty() {
        C::launch_application(&args.app)?;
        return Ok(());
    }

    let focused_window = C::get_focused_window().context("failed to get focused window")?;
    let target_window = get_cycle_window_target::<C>(focused_window, &sibling_windows);

    C::focus_window(target_window).context("failed to focus window")?;

    Ok(())
}

/// Get the window to cycle to next based on `current_focused_window` and its siblings in
/// `sibling_windows`
fn get_cycle_window_target<C: Compositor>(
    current_focused_window: Option<C::Win>,
    sibling_windows: &[C::Win],
) -> &C::Win {
    let current_window_position = current_focused_window.as_ref().and_then(|focused_window| {
        sibling_windows
            .iter()
            .position(|sibling_window| sibling_window == focused_window)
    });

    let target_index = match current_window_position {
        // Already on a matching window: cycle to next most recently focused
        Some(position) => (position + 1) % sibling_windows.len(),
        // Not on a matching window: pick most recent
        None => 0,
    };

    &sibling_windows[target_index]
}

impl CompositorInstance {
    fn run_raisin(&self) -> anyhow::Result<()> {
        match self {
            Self::Hyprland => run::<HyprlandCompositor>(),
            Self::Niri => run::<NiriCompositor>(),
        }
    }
}

fn main() -> anyhow::Result<()> {
    detect_compositor()
        .context("failed to detect compositor type")?
        .run_raisin()
}
