//! Talks to Hyprland over its two IPC sockets: one that answers requests, one
//! that streams events.
//!
//! Going through the sockets rather than spawning `hyprctl` keeps a switch off
//! the process-spawning path entirely, which is what lets a key press turn
//! into a focused window in about a millisecond.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Context, Result};

use crate::bindings::BINDINGS;
use crate::compositor::{self, Window};

/// The event Hyprland emits when a mapped key is pressed, followed by the
/// letter that was pressed.
pub(crate) const SWITCH_EVENT: &str = "raisin:switch:";

/// The event Hyprland emits when Super is released.
pub(crate) const CONFIRM_EVENT: &str = "raisin:confirm";

/// The event Hyprland emits when Escape is pressed while the switcher is on
/// screen.
pub(crate) const CANCEL_EVENT: &str = "raisin:cancel";

/// The Super keys a switch can be confirmed with.
const SUPER_KEYS: [&str; 2] = ["Super_L", "Super_R"];

pub struct Compositor;

/// A window, the way Hyprland's `clients` and `activewindow` describe one.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    address: String,
    class: String,
    title: String,
    mapped: bool,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: i32,
}

impl From<Client> for Window {
    fn from(client: Client) -> Self {
        Self {
            id: client.address,
            app_id: client.class,
            title: client.title,
        }
    }
}

/// Where Hyprland keeps the current instance's sockets.
fn instance_dir() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").context("`$XDG_RUNTIME_DIR` is not set")?;
    let instance = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("`$HYPRLAND_INSTANCE_SIGNATURE` is not set, is Hyprland running?")?;

    Ok(PathBuf::from(runtime_dir).join("hypr").join(instance))
}

/// Sends `command` to Hyprland and returns what it answered.
pub(crate) fn request(command: &str) -> Result<String> {
    let path = instance_dir()?.join(".socket.sock");

    let mut socket = UnixStream::connect(&path)
        .with_context(|| format!("failed to connect to Hyprland at {}", path.display()))?;

    socket
        .write_all(command.as_bytes())
        .context("failed to send a command to Hyprland")?;
    let _ = socket.shutdown(std::net::Shutdown::Write);

    let mut response = String::new();
    socket
        .read_to_string(&mut response)
        .context("failed to read Hyprland's response")?;

    Ok(response)
}

/// Connects to Hyprland's event stream, which reports one event per line.
pub(crate) fn events() -> Result<UnixStream> {
    let path = instance_dir()?.join(".socket2.sock");

    UnixStream::connect(&path).with_context(|| {
        format!(
            "failed to connect to Hyprland's events at {}",
            path.display()
        )
    })
}

/// Which parser Hyprland's configuration is written for. Keybinds added at
/// runtime are spelled differently for each, and each parser refuses the
/// other's spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ConfigLanguage {
    Legacy,
    Lua,
}

/// Asks Hyprland which parser it's using, by sending it a `keyword` command
/// too short to do anything: the Lua parser refuses the command itself, the
/// legacy one gets as far as complaining about the arguments.
pub(crate) fn config_language() -> ConfigLanguage {
    static LANGUAGE: OnceLock<ConfigLanguage> = OnceLock::new();

    *LANGUAGE.get_or_init(|| match request("keyword") {
        Ok(response) if response.contains("non-legacy") => ConfigLanguage::Lua,
        _ => ConfigLanguage::Legacy,
    })
}

/// One of the keybinds the daemon installs.
struct Bind {
    /// The modifiers that have to be held, as Hyprland names them.
    mods: Option<&'static str>,
    /// The key itself, e.g. `i`, `Super_L` or `Escape`.
    key: String,
    /// The event Hyprland emits for it.
    event: String,
    flags: Flags,
}

/// The ways a bind can depart from "fires when the key goes down, and the
/// application never sees it".
#[derive(Default, Clone, Copy)]
struct Flags {
    /// Fires when the key is let go rather than pressed.
    release: bool,
    /// Survives another bind having fired while the key was held. Hyprland
    /// otherwise shadows the bind, which would silence the release of Super
    /// every time the user actually picked a window with it.
    transparent: bool,
    /// Leaves the key to the focused application as well.
    non_consuming: bool,
    /// Fires whatever else is being held down at the time.
    ignore_mods: bool,
}

impl Bind {
    fn add(&self) -> String {
        match config_language() {
            ConfigLanguage::Legacy => format!(
                "keyword bind{} {},event,{}",
                self.flags.letters(),
                self.keys(),
                self.event
            ),
            ConfigLanguage::Lua => format!(
                r#"eval hl.bind("{}", hl.dsp.event("{}"){})"#,
                self.keys(),
                self.event,
                self.flags.options()
            ),
        }
    }

    fn remove(&self) -> String {
        match config_language() {
            ConfigLanguage::Legacy => format!("keyword unbind {}", self.keys()),
            ConfigLanguage::Lua => format!(r#"eval hl.unbind("{}")"#, self.keys()),
        }
    }

    /// The key, spelled the way the configuration language in use spells it:
    /// `SUPER,i` for the legacy parser, `SUPER + i` for the Lua one.
    fn keys(&self) -> String {
        match (config_language(), self.mods) {
            (ConfigLanguage::Legacy, mods) => format!("{},{}", mods.unwrap_or_default(), self.key),
            (ConfigLanguage::Lua, Some(mods)) => format!("{mods} + {}", self.key),
            (ConfigLanguage::Lua, None) => self.key.clone(),
        }
    }
}

impl Flags {
    /// The letters the legacy parser spells these with, as in `bindrtn`.
    fn letters(self) -> String {
        let letters = [
            (self.release, 'r'),
            (self.transparent, 't'),
            (self.non_consuming, 'n'),
            (self.ignore_mods, 'i'),
        ];

        letters
            .into_iter()
            .filter_map(|(set, letter)| set.then_some(letter))
            .collect()
    }

    /// The same, as the table the Lua parser takes.
    fn options(self) -> String {
        let options = [
            (self.release, "release"),
            (self.transparent, "transparent"),
            (self.non_consuming, "non_consuming"),
            (self.ignore_mods, "ignore_mods"),
        ];

        let options: Vec<_> = options
            .into_iter()
            .filter_map(|(set, option)| set.then_some(format!("{option} = true")))
            .collect();

        if options.is_empty() {
            String::new()
        } else {
            format!(", {{ {} }}", options.join(", "))
        }
    }
}

/// The keybinds that make the switcher work, for as long as the daemon runs.
///
/// Hyprland forgets them when its configuration is reloaded, so the daemon
/// puts them back when it sees that happen, and takes them away when it exits.
pub(crate) struct Binds {
    /// Installed for the daemon's lifetime.
    binds: Vec<Bind>,
    /// Installed only while the switcher is on screen, so that Escape reaches
    /// the applications underneath the rest of the time.
    escape: Bind,
}

impl Binds {
    /// Installs a binding for every mapped letter, plus the one that confirms
    /// a switch when Super is released.
    pub(crate) fn install() -> Result<Self> {
        let switches = BINDINGS.iter().map(|binding| Bind {
            mods: Some("SUPER"),
            key: binding.key.to_string(),
            event: format!("{SWITCH_EVENT}{}", binding.key),
            flags: Flags::default(),
        });

        let confirms = SUPER_KEYS.iter().map(|key| Bind {
            mods: Some("SUPER"),
            key: (*key).to_owned(),
            event: CONFIRM_EVENT.to_owned(),
            flags: Flags {
                release: true,
                transparent: true,
                non_consuming: true,
                ..Flags::default()
            },
        });

        let binds = Self {
            binds: switches.chain(confirms).collect(),
            escape: Bind {
                mods: None,
                key: "Escape".to_owned(),
                event: CANCEL_EVENT.to_owned(),
                // Super is still held while the switcher is up, so the bind
                // has to fire regardless of the modifiers.
                flags: Flags {
                    ignore_mods: true,
                    ..Flags::default()
                },
            },
        };
        binds.add()?;

        Ok(binds)
    }

    fn add(&self) -> Result<()> {
        for bind in &self.binds {
            // Drop a leftover from an earlier run first, so reinstalling can't
            // leave the same key bound twice.
            let _ = request(&bind.remove());

            let response = request(&bind.add())?;
            anyhow::ensure!(
                response.trim() == "ok",
                "Hyprland refused the keybind for {}: {}",
                bind.keys(),
                response.trim()
            );
        }

        Ok(())
    }

    /// Takes Escape from the application underneath, for as long as the
    /// switcher is on screen to use it.
    pub(crate) fn capture_escape(&self) {
        if let Err(error) = request(&self.escape.add()) {
            eprintln!("raisin: failed to bind Escape: {error:#}");
        }
    }

    /// Gives Escape back.
    pub(crate) fn release_escape(&self) {
        let _ = request(&self.escape.remove());
    }

    /// Puts the keybinds back after Hyprland reloaded its configuration.
    pub(crate) fn reinstall(&self) {
        if let Err(error) = self.add() {
            eprintln!("raisin: failed to restore keybinds after a config reload: {error:#}");
        }
    }

    /// Takes the keybinds away, leaving Hyprland as it was found.
    pub(crate) fn remove(&self) {
        for bind in &self.binds {
            let _ = request(&bind.remove());
        }

        self.release_escape();
    }
}

impl compositor::Compositor for Compositor {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn get_windows(&self) -> Result<Vec<Window>> {
        let response = request("j/clients")?;
        let mut clients: Vec<Client> =
            serde_json::from_str(&response).context("failed to parse Hyprland's window list")?;

        clients.retain(|client| client.mapped && !client.class.is_empty());
        clients.sort_by_key(|client| client.focus_history_id);

        Ok(clients.into_iter().map(Window::from).collect())
    }

    fn get_focused_window(&self) -> Result<Option<Window>> {
        let response = request("j/activewindow")?;
        let client: serde_json::Value =
            serde_json::from_str(&response).context("failed to parse Hyprland's focused window")?;

        // Hyprland answers with an empty object when nothing is focused.
        if client.get("address").is_none() {
            return Ok(None);
        }

        let client: Client =
            serde_json::from_value(client).context("failed to parse Hyprland's focused window")?;

        Ok(Some(client.into()))
    }

    fn focus_window(&self, window: &Window) -> Result<()> {
        let id = &window.id;
        let command = match config_language() {
            ConfigLanguage::Legacy => format!("dispatch focuswindow address:{id}"),
            ConfigLanguage::Lua => {
                format!(r#"dispatch hl.dsp.focus({{ window = 'address:{id}' }})"#)
            }
        };

        let response = request(&command)?;
        anyhow::ensure!(
            response.trim().starts_with("ok"),
            "Hyprland refused to focus window {id}: {}",
            response.trim()
        );

        Ok(())
    }

    /// Lets Hyprland start the application, so it outlives the daemon and gets
    /// the same treatment as anything else the user launches.
    fn launch_application(&self, cmd: &str) -> Result<()> {
        let command = match config_language() {
            ConfigLanguage::Legacy => format!("dispatch exec {cmd}"),
            ConfigLanguage::Lua => format!(r#"dispatch hl.dsp.exec_cmd("{cmd}")"#),
        };

        let response = request(&command)?;
        anyhow::ensure!(
            response.trim().starts_with("ok"),
            "Hyprland refused to launch '{cmd}': {}",
            response.trim()
        );

        Ok(())
    }

    fn is_running(&self) -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::Compositor as _;

    /// Checks the parsing against whatever Hyprland actually answers, which no
    /// fixture can keep up with. Ignored by default: it needs a running
    /// Hyprland, and only reads from it.
    ///
    /// Run it with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a running Hyprland"]
    fn a_running_hyprland_can_be_read() {
        let windows = Compositor
            .get_windows()
            .expect("failed to read the windows");

        assert!(
            windows.iter().all(|window| !window.app_id.is_empty()),
            "every window should have an app_id: {windows:#?}"
        );

        Compositor
            .get_focused_window()
            .expect("failed to read the focused window");

        println!(
            "{} windows, config language: {:?}",
            windows.len(),
            config_language()
        );
    }
}
