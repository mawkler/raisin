#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about,
    arg_required_else_help = true,
    subcommand_negates_reqs = true,
    after_help = "Examples:\
        \n  raisin ghostty \
        \n  raisin ghostty com.mitchellh.ghostty \
        \n  raisin daemon \
        "
)]
/// Run-or-raise for Hyprland and Niri
pub(crate) struct Args {
    /// Command to run the application (e.g., `ghostty`).
    #[arg(required = true)]
    pub(crate) app: Option<String>,

    /// Window app_id to match (e.g., `com.mitchellh.ghostty`). Optional.
    ///
    /// If omitted, the app name is used as a substring to match against
    /// window class names.
    pub(crate) app_id: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Command {
    /// Run the switcher. Hyprland only.
    ///
    /// Start it once, for example from your Hyprland startup configuration.
    /// It binds Super + a letter for every application it knows about, taking
    /// those keys over from any binding your configuration gives them, and
    /// removes its own bindings again when it stops. `hyprctl reload` puts
    /// your configuration's bindings back.
    Daemon,

    /// Switch to an application through the running switcher.
    Switch {
        /// Command to run the application (e.g., `ghostty`).
        app: String,

        /// Window app_id to match (e.g., `com.mitchellh.ghostty`). Optional.
        app_id: Option<String>,
    },
}
