use std::process::{Command, Output};

use anyhow::{Context, Result};

use crate::compositor::{self};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    address: String,
    class: String,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: u32,
    title: String,
}

fn run_hyprctl_command(args: &[&str]) -> Result<Output> {
    Command::new("hyprctl")
        .args(args)
        .output()
        .with_context(|| format!("failed to run command 'hyprctl {}'", args.join(" ")))
}

pub struct Compositor;

impl compositor::Compositor for Compositor {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn get_windows(&self) -> Result<Vec<compositor::Window>> {
        let output = run_hyprctl_command(&["clients", "-j"])?;
        if !output.status.success() {
            anyhow::bail!(
                "hyprctl clients -j failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut hyprland_windows: Vec<Window> = serde_json::from_slice(&output.stdout)
            .context("failed to parse JSON output of clients command")?;

        hyprland_windows.sort_by_key(|w| std::cmp::Reverse(w.focus_history_id));

        Ok(hyprland_windows
            .into_iter()
            .map(|window| compositor::Window {
                id: window.address,
                app_id: window.class,
                title: window.title,
            })
            .collect())
    }

    fn focus_window(&self, window: &compositor::Window) -> Result<()> {
        let address = &window.id;
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

    fn is_running(&self) -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || Command::new("hyprctl").arg("version").output().is_ok()
    }
}
