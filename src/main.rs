use crate::application::Application;

use crate::compositor::integrations::hyprland::HyprlandCompositor;
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
