use anyhow::Context;
use flexi_logger::{Duplicate, FileSpec, Logger};
use std::ffi::OsStr;

/// Logs to stederr and `log_file` if provided, otherwise only to stderr
pub(crate) fn init(log_file: Option<&std::path::Path>) -> anyhow::Result<()> {
    let logger = Logger::try_with_env_or_str("info").unwrap();

    let Some(path) = log_file else {
        logger
            .log_to_stderr()
            .start()
            .context("failed to start logging to stderr")?;
        return Ok(());
    };

    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("raisin");

    let mut file_spec = FileSpec::default()
        .directory(dir)
        .basename(stem)
        .suppress_timestamp();

    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        file_spec = file_spec.suffix(ext);
    }

    logger
        .log_to_file(file_spec.clone())
        .duplicate_to_stderr(Duplicate::All)
        .start()
        .context(format!(
            "failed to start logging to file {file_spec:?} and stderr",
        ))?;

    Ok(())
}
