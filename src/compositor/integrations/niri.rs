use crate::compositor::{Compositor, Window};
use anyhow::{Context, Result};
use std::process::{Command, Output};

#[derive(Debug, Clone, serde::Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Timestamp {
    secs: u64,
    nanos: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NiriWindow {
    id: u32,
    app_id: String,
    focus_timestamp: Timestamp,
    title: String,
}

// `Eq` implemented manually to compare only the window ID
impl PartialEq for NiriWindow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for NiriWindow {}

impl Window for NiriWindow {
    type Timestamp = Timestamp;

    fn app_id(&self) -> &str {
        &self.app_id
    }

    fn last_focused(&self) -> &Self::Timestamp {
        &self.focus_timestamp
    }

    fn title(&self) -> &str {
        &self.title
    }
}

fn run_niri_command(args: &[&str]) -> Result<Output> {
    Command::new("niri")
        .args(args)
        .output()
        .with_context(|| format!("failed to run command 'niri {}'", args.join(" ")))
}

pub struct NiriCompositor;

impl Compositor for NiriCompositor {
    type Win = NiriWindow;

    fn is_running() -> bool {
        std::env::var("NIRI_SOCKET").is_ok()
            || Command::new("niri").arg("--version").output().is_ok()
    }

    fn get_windows() -> Result<Vec<Self::Win>> {
        let windows_output = run_niri_command(&["msg", "--json", "windows"])?;
        let windows: Vec<NiriWindow> = serde_json::from_slice(&windows_output.stdout)
            .context("failed to parse JSON output of window command")?;
        Ok(windows)
    }

    fn get_focused_window() -> Result<Option<Self::Win>> {
        let focused_window_json = run_niri_command(&["msg", "--json", "focused-window"])?;
        let focused_window_json = String::from_utf8_lossy(&focused_window_json.stdout);
        let focused_window: Option<NiriWindow> = serde_json::from_str(&focused_window_json)
            .context("failed to parse focused window JSON")?;
        Ok(focused_window)
    }

    fn focus_window(window: &Self::Win) -> Result<()> {
        let id = window.id.to_string();
        let _ = Command::new("niri")
            .args(["msg", "action", "focus-window", "--id", &id])
            .spawn()
            .with_context(|| format!("failed to focus window id {id}"))?
            .wait();
        Ok(())
    }
}
