use crate::compositor::integrations::hyprland::HyprlandCompositor;
use crate::compositor::integrations::niri::NiriCompositor;
use anyhow::Context;
use clap::Parser;
use compositor::{Compositor, Window};
use std::cmp::Reverse;

mod cli;
mod compositor;

/// Detect which compositor is running
fn detect_compositor() -> Option<String> {
    // Check environment variable first
    if let Ok(compositor) = std::env::var("RAISIN_COMPOSITOR") {
        return Some(compositor);
    }

    // TODO: is there a way to iterate over all `Compositors` and pick the first one that's running?
    // Check each compositor using is_running()
    if HyprlandCompositor::is_running() {
        return Some("hyprland".to_string());
    }
    if NiriCompositor::is_running() {
        return Some("niri".to_string());
    }

    None
}

fn run<C: Compositor>() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    // Filter open windows by app_class (case-insensitive)
    let windows = C::get_windows().context("failed to get windows")?;
    let mut matching_windows: Vec<_> = windows
        .into_iter()
        .filter(|window| {
            window
                .app_id()
                .to_lowercase()
                // TODO: add option to do strict matching
                .contains(&args.app_class.to_lowercase())
        })
        .collect();

    // Sort windows by last focused timestamp
    matching_windows.sort_by_key(|w| Reverse(w.last_focused().clone()));

    // If no matching window is found, launch the app
    if matching_windows.is_empty() {
        let app = args.app_cmd.unwrap_or_else(|| args.app_class.clone());
        C::launch_application(&app)?;
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

fn main() -> anyhow::Result<()> {
    let compositor = detect_compositor().context("failed to detect compositor type")?;

    match compositor.as_str() {
        "hyprland" => run::<HyprlandCompositor>(),
        "niri" => run::<NiriCompositor>(),
        _ => anyhow::bail!("unsupported compositor: '{compositor}'"),
    }
}
