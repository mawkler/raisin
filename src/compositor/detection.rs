use crate::compositor::Compositor;
use crate::compositor::integrations::hyprland::HyprlandCompositor;
use crate::compositor::integrations::niri::NiriCompositor;

pub(crate) enum CompositorInstance {
    Hyprland,
    Niri,
}

impl CompositorInstance {
    pub(crate) fn is_running(&self) -> bool {
        match self {
            Self::Hyprland => HyprlandCompositor::is_running(),
            Self::Niri => NiriCompositor::is_running(),
        }
    }

    pub(crate) fn all() -> [CompositorInstance; 2] {
        [Self::Hyprland, Self::Niri]
    }
}

impl std::str::FromStr for CompositorInstance {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hyprland" => Ok(Self::Hyprland),
            "niri" => Ok(Self::Niri),
            other => anyhow::bail!("unsupported compositor: '{other}'"),
        }
    }
}

pub(crate) fn detect_compositor() -> Option<CompositorInstance> {
    if let Ok(compositor) = std::env::var("RAISIN_COMPOSITOR") {
        return compositor.parse().ok();
    }

    CompositorInstance::all()
        .into_iter()
        .find(|instance| instance.is_running())
}
