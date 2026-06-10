use std::process::{Command, Output};

use anyhow::{Context, Result};

use crate::compositor;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    address: String,
    class: String,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: u32,
    title: String,
}

impl compositor::Window for Window {
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

pub struct Compositor;

impl compositor::Compositor for Compositor {
    type Win = Window;

    fn get_windows() -> Result<Vec<Self::Win>> {
        let output = run_hyprctl_command(&["clients", "-j"])?;
        if !output.status.success() {
            anyhow::bail!(
                "hyprctl clients -j failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut windows: Vec<Window> = serde_json::from_slice(&output.stdout)
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

    fn is_running() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || Command::new("hyprctl").arg("version").output().is_ok()
    }
}
