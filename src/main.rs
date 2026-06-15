use crate::application::Application;
use clap::Parser;

mod application;
mod cli;
mod compositor;
mod gui;

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();
    let compositor = compositor::detect()?;

    Application::new(args, compositor).run()
}
