use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

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

    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let cmd = json!({"cycle": direction});
            serde_json::to_writer(&mut stream, &cmd)?;
            writeln!(&mut stream)?;
            Ok(true)
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionRefused => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn start_listener() -> Result<mpsc::Receiver<String>> {
    let path = socket_path();

    if path.exists() {
        std::fs::remove_file(&path).context("failed to remove stale socket")?;
    }

    let listener = std::os::unix::net::UnixListener::bind(&path)
        .context("failed to bind raisin socket")?;
    listener
        .set_nonblocking(true)
        .context("failed to set socket non-blocking")?;

    let (cmd_tx, cmd_rx) = mpsc::channel();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => break,
            };

            let line = BufReader::new(&mut stream)
                .lines()
                .next()
                .and_then(Result::ok)
                .unwrap_or_default();

            let _ = cmd_tx.send(line);
        }
    });

    Ok(cmd_rx)
}
