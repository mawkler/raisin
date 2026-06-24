use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about,
    arg_required_else_help = true,
    after_help = "Examples:\
        \n  raisin ghostty \
        \n  raisin ghostty com.mitchellh.ghostty \
        \n  raisin --gui --trigger-key t ghostty \
        "
)]
/// Run-or-raise for Hyprland and Niri
pub(crate) struct Args {
    /// Command to run the application (e.g., `ghostty`).
    pub(crate) app: String,

    /// Window app ID to match (e.g., `com.mitchellh.ghostty`). Optional.
    ///
    /// If omitted, the app name is used as a substring to match against
    /// window class names.
    pub(crate) app_id: Option<String>,

    /// Show GUI window switcher.
    #[arg(long)]
    pub(crate) gui: bool,

    /// Key used to trigger this invocation (for cycling windows in GUI mode).
    #[arg(long)]
    pub(crate) trigger_key: Option<char>,

    /// Path to write logs to (in addition to stderr).
    #[arg(long)]
    pub(crate) log_file: Option<PathBuf>,
}
