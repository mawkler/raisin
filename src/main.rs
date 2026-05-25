use anyhow::Context;
use clap::Parser;
use compositor::detection::CompositorInstance;
use compositor::{Compositor, Window};
use std::cmp::Reverse;

use crate::compositor::{
    detection::detect_compositor,
    integrations::{hyprland::HyprlandCompositor, niri::NiriCompositor},
};

mod cli;
mod compositor;

fn run<C: Compositor>() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    // Filter open windows by app_id (case-insensitive)
    let search_string = args.app_id.as_deref().unwrap_or(&args.app).to_lowercase();
    let windows = C::get_windows().context("failed to get windows")?;
    let mut matching_windows: Vec<_> = windows
        .into_iter()
        .filter(|window| {
            window
                .app_id()
                .to_lowercase()
                // TODO: add option to do strict matching
                .contains(&search_string)
        })
        .collect();

    // Sort windows by last focused timestamp
    matching_windows.sort_by_key(|w| Reverse(w.last_focused().clone()));

    // If no matching window is found, launch the app
    if matching_windows.is_empty() {
        C::launch_application(&args.app)?;
        return Ok(());
    }

    // Otherwise, find the currently focused window
    let focused_window = C::get_focused_window().context("failed to get focused window")?;

    // Find the position of the currently focused window relative to its program's other windows
    let current_window_position = focused_window.as_ref().and_then(|focused_window| {
        matching_windows
            .iter()
            .position(|matching_window| matching_window == focused_window)
    });

    let target_index = match current_window_position {
        // Already on a matching window: cycle to next most recently focused
        Some(position) => (position + 1) % matching_windows.len(),
        // Not on a matching window: pick most recent
        None => 0,
    };

    C::focus_window(&matching_windows[target_index]).context("failed to focus window")?;

    Ok(())
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
    let compositor = detect_compositor().context("failed to detect compositor type")?;
    compositor.run_raisin()
}
