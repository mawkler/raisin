use std::io::Write;
use std::os::unix::net::{self};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::picker::Direction;

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    PathBuf::from(runtime).join(format!("raisin-{display}.sock"))
}

pub(crate) fn try_send(direction: Direction) -> Result<bool> {
    let path = socket_path();

    if !path.exists() {
        return Ok(false);
    }

    match net::UnixStream::connect(&path) {
        Ok(mut stream) => {
            writeln!(stream, "{direction}").context("failed to write to Unix socket")?;
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => Ok(false),
        Err(err) => anyhow::bail!("failed to connect to Unix socket: {err}"),
    }
}

pub(crate) fn start_listener() -> Result<net::UnixListener> {
    let path = socket_path();

    if path.exists() {
        std::fs::remove_file(&path).context("failed to remove stale socket")?;
    }

    let listener = net::UnixListener::bind(&path).context("failed to bind Unix socket")?;
    listener
        .set_nonblocking(true)
        .context("failed to make socket listener non-blocking")?;

    Ok(listener)
}
