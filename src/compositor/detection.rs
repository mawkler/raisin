use anyhow::Context;
use enum_dispatch::enum_dispatch;

use super::{Compositor, integrations};

/// A `Compositor` wrapper with all its implementation variants
#[enum_dispatch(Compositor)]
pub(crate) enum ActiveCompositor {
    Hyprland(integrations::hyprland::Compositor),
    Niri(integrations::niri::Compositor),
}

const COMPOSITORS: [ActiveCompositor; 2] = [
    ActiveCompositor::Hyprland(integrations::hyprland::Compositor),
    ActiveCompositor::Niri(integrations::niri::Compositor),
];

const COMPOSITOR_ENV_VAR: &str = "RAISIN_COMPOSITOR";

pub(crate) fn detect() -> anyhow::Result<ActiveCompositor> {
    if let Ok(env_compositor) = std::env::var(COMPOSITOR_ENV_VAR) {
        let compositor = COMPOSITORS
            .into_iter()
            .find(|compositor| compositor.name() == env_compositor.to_lowercase())
            .with_context(|| {
                format!(
                    "`{COMPOSITOR_ENV_VAR}` is set to '{env_compositor}', \
                     which is not a supported compositor"
                )
            });

        log::info!("`{COMPOSITOR_ENV_VAR}` is set to {env_compositor}, using that as compositor");
        return compositor;
    }

    let compositor = COMPOSITORS
        .into_iter()
        .find(Compositor::is_running)
        .context("could not find any supported compositor running")?;

    log::info!("detected compositor: {name}", name = compositor.name());
    Ok(compositor)
}
