use std::process::{Command, Output};

use anyhow::{Context, Result};

use crate::compositor::{self};

#[derive(Debug, Clone, serde::Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct Timestamp {
    secs: u64,
    nanos: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Window {
    id: u32,
    app_id: String,
    focus_timestamp: Timestamp,
    title: String,
}

fn run_niri_command(args: &[&str]) -> Result<Output> {
    Command::new("niri")
        .args(args)
        .output()
        .with_context(|| format!("failed to run command 'niri {}'", args.join(" ")))
}

pub struct Compositor;

impl compositor::Compositor for Compositor {
    fn name(&self) -> &'static str {
        "niri"
    }

    fn get_focused_window(&self) -> Result<Option<compositor::Window>> {
        let focused_window_json = run_niri_command(&["msg", "--json", "focused-window"])?;
        if !focused_window_json.status.success() {
            let err = String::from_utf8_lossy(&focused_window_json.stderr);
            anyhow::bail!("niri msg --json focused-window failed: {err}");
        }

        let focused_window_json = String::from_utf8_lossy(&focused_window_json.stdout);
        let focused_window: Option<Window> = serde_json::from_str(&focused_window_json)
            .context("failed to parse focused window JSON")?;

        Ok(focused_window.map(|window| compositor::Window {
            id: window.id.to_string(),
            app_id: window.app_id,
            title: window.title,
        }))
    }

    fn get_windows(&self) -> Result<Vec<compositor::Window>> {
        let windows_output = run_niri_command(&["msg", "--json", "windows"])?;
        if !windows_output.status.success() {
            anyhow::bail!(
                "niri msg --json windows failed: {}",
                String::from_utf8_lossy(&windows_output.stderr)
            );
        }
        let mut niri_windows: Vec<Window> = serde_json::from_slice(&windows_output.stdout)
            .context("failed to parse JSON output of window command")?;
        niri_windows.sort_by_key(|w| std::cmp::Reverse(w.focus_timestamp.clone()));

        let windows = niri_windows
            .into_iter()
            .map(|window| compositor::Window {
                id: window.id.to_string(),
                app_id: window.app_id,
                title: window.title,
            })
            .collect();

        Ok(windows)
    }

    fn focus_window(&self, window: &compositor::Window) -> Result<()> {
        let id = &window.id;
        let output = Command::new("niri")
            .args(["msg", "action", "focus-window", "--id", id])
            .output()
            .with_context(|| format!("failed to run niri focus-window for {id}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            log::warn!("failed to focus window {id}: {stdout}{stderr}");
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        std::env::var("NIRI_SOCKET").is_ok()
            || Command::new("niri").arg("--version").output().is_ok()
    }
}
