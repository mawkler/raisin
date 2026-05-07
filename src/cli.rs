#[derive(clap::Parser, Debug)]
#[command(
    author,
    version,
    about,
    arg_required_else_help = true,
    after_help = "Examples:\
        \n  raisin ghostty \
        \n  raisin brave-browser brave \
        "
)]
/// Run or raise
pub(crate) struct Args {
    /// Application's app_id (e.g., `com.mitchellh.ghostty`).
    ///
    /// Will do partial matching.
    pub(crate) app_class: String,

    /// Command to run the application (e.g., `ghostty`). Optional.
    ///
    /// If omitted, use `app_class`.
    pub(crate) app_cmd: Option<String>,
}
