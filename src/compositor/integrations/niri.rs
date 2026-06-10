use std::process::{Command, Output};

use anyhow::{Context, Result};

use crate::compositor;

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

impl compositor::Window for Window {
    fn app_id(&self) -> &str {
        &self.app_id
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

pub struct Compositor;

impl compositor::Compositor for Compositor {
    type Win = Window;

    fn get_windows() -> Result<Vec<Self::Win>> {
        let windows_output = run_niri_command(&["msg", "--json", "windows"])?;
        if !windows_output.status.success() {
            anyhow::bail!(
                "niri msg --json windows failed: {}",
                String::from_utf8_lossy(&windows_output.stderr)
            );
        }
        let mut windows: Vec<Window> = serde_json::from_slice(&windows_output.stdout)
            .context("failed to parse JSON output of window command")?;
        windows.sort_by_key(|w| std::cmp::Reverse(w.focus_timestamp.clone()));
        Ok(windows)
    }

    fn focus_window(window: &Self::Win) -> Result<()> {
        let id = window.id.to_string();
        let output = Command::new("niri")
            .args(["msg", "action", "focus-window", "--id", &id])
            .output()
            .with_context(|| format!("failed to run niri focus-window for {id}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!("warning: failed to focus window {id}: {stdout}{stderr}");
        }
        Ok(())
    }

    fn is_running() -> bool {
        std::env::var("NIRI_SOCKET").is_ok()
            || Command::new("niri").arg("--version").output().is_ok()
    }
}
