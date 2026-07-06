use std::io::Write;
use std::os::unix::net::{self};
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    PathBuf::from(runtime).join(format!("raisin-{display}.sock"))
}

pub(crate) fn try_send(direction: &str) -> Result<bool> {
    let path = socket_path();

    if !path.exists() {
        return Ok(false);
    }

    match net::UnixStream::connect(&path) {
        Ok(mut stream) => {
            let cmd = json!({"cycle": direction});
            serde_json::to_writer(&mut stream, &cmd)?;
            writeln!(&mut stream).context("failed to write to Unix socket")?;
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
        .context("failed to make socket non-blocking")?;

    Ok(listener)
}
