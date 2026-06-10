use anyhow::Context;

use crate::application::Application;

mod application;
mod cli;
mod compositor;

fn main() -> anyhow::Result<()> {
    match compositor::detect().context("no supported compositor detected")? {
        compositor::Active::Hyprland => Application::<compositor::hyprland::Compositor>::run(),
        compositor::Active::Niri => Application::<compositor::niri::Compositor>::run(),
    }
}
