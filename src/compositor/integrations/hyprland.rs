use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::process::{Command, Output};

use crate::compositor::{Compositor, Window};

// Hyprland uses a relative order for focus history rather than timestamps
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct FocusOrder(u32);

impl PartialOrd for FocusOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FocusOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprlandWindow {
    address: String,
    class: String,
    focus_history_id: FocusOrder,
    title: String,
}

impl PartialEq for HyprlandWindow {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl Eq for HyprlandWindow {}

impl Window for HyprlandWindow {
    type Timestamp = FocusOrder;

    fn app_id(&self) -> &str {
        &self.class
    }

    fn last_focused(&self) -> &Self::Timestamp {
        &self.focus_history_id
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

    fn is_running() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || Command::new("hyprctl").arg("version").output().is_ok()
    }

    fn get_windows() -> Result<Vec<Self::Win>> {
        let output = run_hyprctl_command(&["clients", "-j"])?;
        if !output.status.success() {
            anyhow::bail!(
                "hyprctl clients -j failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let windows: Vec<HyprlandWindow> = serde_json::from_slice(&output.stdout)
            .context("failed to parse JSON output of clients command")?;
        Ok(windows)
    }

    fn get_focused_window() -> Result<Option<Self::Win>> {
        let output = run_hyprctl_command(&["activewindow", "-j"])?;
        if !output.status.success() {
            anyhow::bail!(
                "hyprctl activewindow -j failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let output = String::from_utf8_lossy(&output.stdout);
        let trimmed = output.trim();

        // No window focused
        if trimmed.is_empty() || trimmed == "{}" {
            return Ok(None);
        }

        let window: HyprlandWindow =
            serde_json::from_str(trimmed).context("failed to parse active window's JSON")?;
        Ok(Some(window))
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
