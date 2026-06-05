use anyhow::{Context, Result};
use std::process::{Command, Output};

use crate::compositor::{Compositor, Window};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprlandWindow {
    address: String,
    class: String,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: u32,
    #[allow(dead_code)]
    title: String,
}

impl Window for HyprlandWindow {
    fn app_id(&self) -> &str {
        &self.class
    }

    fn title(&self) -> &str {
        &self.title
    }
}

fn run_hyprctl_command(args: &[&str]) -> Result<Output> {
    Command::new("hyprctl")
        .args(args)
        .output()
        .with_context(|| format!("failed to run command 'hyprctl {}'", args.join(" ")))
}

pub struct HyprlandCompositor;

impl Compositor for HyprlandCompositor {
    type Win = HyprlandWindow;

    fn get_windows() -> Result<Vec<Self::Win>> {
        let output = run_hyprctl_command(&["clients", "-j"])?;
        if !output.status.success() {
            anyhow::bail!(
                "hyprctl clients -j failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut windows: Vec<HyprlandWindow> = serde_json::from_slice(&output.stdout)
            .context("failed to parse JSON output of clients command")?;

        windows.sort_by_key(|w| std::cmp::Reverse(w.focus_history_id));
        Ok(windows)
    }

    fn focus_window(window: &Self::Win) -> Result<()> {
        let address = &window.address;
        let focus = &format!("hl.dsp.focus({{ window = 'address:{address}' }})");
        let output = Command::new("hyprctl")
            .args(["dispatch", focus])
            .output()
            .with_context(|| format!("failed to run hyprctl dispatch for {address}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.starts_with("warning:") {
            eprintln!("got warning when focusing window {address}: {stdout}");
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("failed to focus window {address}: {stdout}{stderr}");
        }

        Ok(())
    }
}
