use crate::application::Application;

#[cfg(feature = "compositor-hyprland")]
use crate::compositor::integrations::hyprland::HyprlandCompositor;
#[cfg(feature = "compositor-niri")]
use crate::compositor::integrations::niri::NiriCompositor;

mod application;
mod cli;
mod compositor;

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "compositor-hyprland")]
    return Application::<HyprlandCompositor>::run();

    #[cfg(feature = "compositor-niri")]
    return Application::<NiriCompositor>::run();
}
