use anyhow::Context;
use clap::Parser;
use compositor::Compositor;

#[cfg(feature = "hyprland")]
use crate::compositor::integrations::hyprland::HyprlandCompositor;
#[cfg(feature = "niri")]
use crate::compositor::integrations::niri::NiriCompositor;

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

    // `get_windows` returns windows sorted most-recently-focused first.
    // Pick the second-most-recent window, or the only one if alone.
    let target_window = if sibling_windows.len() >= 2 {
        &sibling_windows[1]
    } else {
        &sibling_windows[0]
    };

    C::focus_window(target_window).context("failed to focus window")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "hyprland")]
    return run::<HyprlandCompositor>();

    #[cfg(feature = "niri")]
    return run::<NiriCompositor>();
}
