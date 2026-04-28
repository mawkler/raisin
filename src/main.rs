use anyhow::Context;
use clap::Parser;
use serde::Deserialize;
use std::cmp::Reverse;
use std::process::{Command, Output};

#[derive(Debug, Clone, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
struct Timestamp {
    secs: u64,
    nanos: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Window {
    id: u32,
    app_id: String,
    last_focused: Timestamp,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    arg_required_else_help = true,
    after_help = "Examples:\
        \n  raisin ghostty \
        \n  raisin brave-browser brave \
        "
)]
/// Run or raise
struct Args {
    /// Application's app_id (e.g., `com.mitchellh.ghostty`).
    ///
    /// Will do partial matching.
    app_class: String,

    /// Command to run the application (e.g., `ghostty`). Optional.
    ///
    /// If omitted, use `app_class`.
    app_cmd: Option<String>,
}

/// Run a `niri` command and return its output
fn run_niri_command(args: &[&str]) -> anyhow::Result<Output> {
    Command::new("niri")
        .args(args)
        .output()
        .with_context(|| format!("failed to run command 'niri {}'", args.join(" ")))
}

/// Focus a window by its ID
fn focus_window_by_id(window_id: u32) -> anyhow::Result<()> {
    let _ = Command::new("niri")
        .args([
            "msg",
            "action",
            "focus-window",
            "--id",
            &window_id.to_string(),
        ])
        .spawn()
        .with_context(|| format!("failed to focus window id {window_id}"))?
        .wait();

    Ok(())
}

/// Launch an application by its command
fn launch_application(app_cmd: &str) -> anyhow::Result<()> {
    let _ = Command::new(app_cmd)
        .spawn()
        .with_context(|| format!("failed to launch application '{app_cmd}'"))?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Get all windows
    let windows_output = run_niri_command(&["msg", "-j", "windows"])?;
    let windows: Vec<Window> =
        serde_json::from_slice(&windows_output.stdout).context("failed to parse windows JSON")?;

    // Filter windows by app_class (case-insensitive), then sort by `last_focused` descending (most
    // recently focused first) so cycling visits windows in recency order
    let mut matching_windows: Vec<Window> = windows
        .into_iter()
        .filter(|window| {
            window
                .app_id
                .to_lowercase()
                // TODO: add option to do strict matching
                .contains(&args.app_class.to_lowercase())
        })
        .collect();

    matching_windows.sort_by_key(|w| Reverse(w.last_focused.clone()));

    // No matching windows found - launch the app
    if matching_windows.is_empty() {
        let app = args.app_cmd.unwrap_or_else(|| args.app_class.clone());
        launch_application(&app)?;
        return Ok(());
    }

    // Otherwise, find the currently focused window
    let focused_window_json = run_niri_command(&["msg", "-j", "focused-window"])?;
    let focused_window_json = String::from_utf8_lossy(&focused_window_json.stdout);
    let focused_window: Option<Window> = serde_json::from_str(&focused_window_json)
        .context("failed to parse focused window JSON")?;

    const FIRST_WINDOW_INDEX: usize = 0;

    let target_window_index = match focused_window {
        None => FIRST_WINDOW_INDEX,
        Some(window) => matching_windows
            .iter()
            // If we're already focusing a window of the same `app_class`
            .position(|w| w.id == window.id)
            // Otherwise, pick the first window
            .unwrap_or(FIRST_WINDOW_INDEX),
    };

    // Cycle to the next window of the same app_class (most recently visited first)
    let target_index = (target_window_index + 1) % matching_windows.len();
    focus_window_by_id(matching_windows[target_index].id)?;

    Ok(())
}
