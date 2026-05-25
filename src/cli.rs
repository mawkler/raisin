#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about,
    arg_required_else_help = true,
    after_help = "Examples:\
        \n  raisin ghostty \
        \n  raisin ghostty com.mitchellh.ghostty \
        "
)]
/// Run or raise
pub(crate) struct Args {
    /// Command to run the application (e.g., `ghostty`).
    pub(crate) app: String,

    /// Window app_id to match (e.g., `com.mitchellh.ghostty`). Optional.
    ///
    /// If omitted, the app name is used as a substring to match against
    /// window class names.
    pub(crate) app_id: Option<String>,
}
