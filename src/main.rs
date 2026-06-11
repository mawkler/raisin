use crate::application::Application;

mod application;
mod cli;
mod compositor;

fn main() -> anyhow::Result<()> {
    let compositor = compositor::detect()?;
    Application::run(compositor)
}
