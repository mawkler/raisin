use anyhow::Context;

pub(crate) fn init(log_file: Option<&std::path::Path>) -> anyhow::Result<()> {
    let level = std::env::var("RUST_LOG")
        .unwrap_or("info".into())
        .parse::<log::LevelFilter>()
        .unwrap_or(log::LevelFilter::Info);

    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            let format = chrono::Local::now().format("%H:%M:%S");
            let level = record.level();
            let target = record.target();
            out.finish(format_args!("{format} [{level}][{target}] {message}"));
        })
        .level(level)
        .chain(std::io::stderr());

    if let Some(path) = log_file {
        let file = fern::log_file(path).context("failed to create log file")?;
        dispatch = dispatch.chain(file);
    }

    dispatch.apply().context("failed to initialize logging")?;

    Ok(())
}
