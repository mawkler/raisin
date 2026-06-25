use crate::application::Application;
use clap::Parser;

mod application;
mod cli;
mod compositor;
mod gui;
mod input;
mod logging;
mod state;

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    logging::init(args.log_file.as_deref())?;

    log::debug!("starting raisin with args: {args:#?}");

    let compositor = compositor::detect()?;

    Application::new(args, compositor).run()
}
