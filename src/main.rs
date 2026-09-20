use clap::Parser;

use crate::application::Application;

mod application;
mod bindings;
mod cli;
mod compositor;
mod daemon;
mod switcher;

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    match args.command {
        Some(cli::Command::Daemon) => daemon::run(),
        Some(cli::Command::Switch { app, app_id }) => daemon::switch(&app, app_id.as_deref()),
        None => {
            let app = args
                .app
                .as_deref()
                .expect("clap requires an application when there's no subcommand");
            let compositor = compositor::detect()?;

            Application::new(compositor, app, args.app_id.as_deref()).run()
        }
    }
}
