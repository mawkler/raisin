use anyhow::Context;

use crate::compositor::{Compositor, hyprland, niri};

pub(crate) enum Active {
    Hyprland,
    Niri,
}

impl Active {
    pub(crate) fn is_active(&self) -> bool {
        match self {
            Self::Hyprland => hyprland::Compositor::is_running(),
            Self::Niri => niri::Compositor::is_running(),
        }
    }

    pub(crate) fn all() -> [Active; 2] {
        [Self::Hyprland, Self::Niri]
    }
}

impl std::str::FromStr for Active {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hyprland" => Ok(Self::Hyprland),
            "niri" => Ok(Self::Niri),
            other => anyhow::bail!("unsupported compositor: '{other}'"),
        }
    }
}

pub(crate) fn detect() -> anyhow::Result<Active> {
    if let Ok(compositor) = std::env::var("RAISIN_COMPOSITOR") {
        return compositor.parse().context(format!(
            "`$RAISIN_COMPOSITOR` is set to '{compositor}', which is not a supported compositor"
        ));
    }

    Active::all()
        .into_iter()
        .find(Active::is_active)
        .context("could not find any supported compositor running")
}
