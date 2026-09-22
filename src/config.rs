//! The configuration file: which key targets which application, which keys
//! steer the switcher once it's up, and the few knobs it exposes.
//!
//! Everything has a default, so raisin runs without a configuration file at
//! all. Writing one replaces the defaults it mentions and leaves the rest.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

/// Where the configuration lives unless `--config` says otherwise.
pub(crate) fn default_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    Some(config_home.join("raisin").join("config.toml"))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Every key raisin binds.
    #[serde(default)]
    pub(crate) keys: Keys,
    /// How the switcher behaves and how big it is.
    #[serde(default)]
    pub(crate) switcher: Switcher,
    /// The thumbnails of the windows being switched between.
    #[serde(default)]
    pub(crate) previews: Previews,
}

impl Config {
    /// The file [`Self::load`] reads, which is also the file to watch for
    /// changes.
    pub(crate) fn path(explicit: Option<&Path>) -> Option<PathBuf> {
        explicit.map(Path::to_owned).or_else(default_path)
    }

    /// Reads the configuration, falling back to the defaults when there isn't
    /// one to read.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` was given but can't be read, or if the file
    /// doesn't parse.
    pub(crate) fn load(path: Option<&Path>) -> Result<Self> {
        let required = path.is_some();

        let Some(path) = Self::path(path) else {
            return Ok(Self::default());
        };

        let file = match std::fs::read_to_string(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        };

        toml::from_str(&file).with_context(|| format!("{} isn't valid", path.display()))
    }

    /// The application the key raisin named `id` targets, if any.
    pub(crate) fn target(&self, id: &str) -> Option<&Target> {
        self.keys
            .apps
            .iter()
            .find(|(key, _)| key.id() == id)
            .map(|(_, target)| target)
    }
}

/// An application the switcher can target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "Entry")]
pub(crate) struct Target {
    /// The command that launches it, e.g. `ghostty`.
    pub(crate) app: String,
    /// The window class its windows carry, when that isn't the command name.
    pub(crate) app_id: Option<String>,
}

impl Target {
    pub(crate) fn new(app: &str, app_id: Option<&str>) -> Self {
        Self {
            app: app.to_owned(),
            app_id: app_id.map(str::to_owned),
        }
    }

    /// What to match the open windows against.
    pub(crate) fn search(&self) -> &str {
        self.app_id.as_deref().unwrap_or(&self.app)
    }
}

/// An application as the configuration file spells it: either the bare command
/// to run, or a table when its windows carry a different class.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    Command(String),
    Detailed {
        cmd: String,
        #[serde(default)]
        app_id: Option<String>,
    },
}

impl From<Entry> for Target {
    fn from(entry: Entry) -> Self {
        match entry {
            Entry::Command(app) => Self { app, app_id: None },
            Entry::Detailed { cmd, app_id } => Self { app: cmd, app_id },
        }
    }
}

/// Every key raisin binds: one per application, plus the ones that steer a
/// switch already in progress.
///
/// The steering keys are bound only while the switcher is on screen, so they
/// belong to the applications underneath the rest of the time.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Keys {
    /// Which key targets which application. `Super` and the key starts a
    /// switch; for a key with no modifiers of its own, holding `Shift` as well
    /// walks the group the other way.
    ///
    /// A key may carry further modifiers, spelled as `next` and `previous`
    /// are: `"SHIFT + a"`.
    #[serde(default)]
    pub(crate) apps: BTreeMap<Key, Target>,
    /// Moves the highlight to the next window.
    #[serde(default)]
    pub(crate) next: Option<Key>,
    /// Moves it back to the previous one.
    #[serde(default)]
    pub(crate) previous: Option<Key>,
    /// Closes the switcher without switching.
    #[serde(default = "default_cancel")]
    pub(crate) cancel: Key,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            apps: BTreeMap::new(),
            next: None,
            previous: None,
            cancel: default_cancel(),
        }
    }
}

/// A key, as the configuration file spells it: `Tab`, or `SHIFT + Tab`.
///
/// Super is held throughout a switch, so it's implied and needn't be written.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Key {
    /// Modifiers besides Super, as Hyprland names them.
    pub(crate) mods: Vec<String>,
    /// The key itself, as libxkbcommon names it.
    pub(crate) key: String,
}

impl FromStr for Key {
    type Err = String;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let mut parts: Vec<_> = key
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();

        let Some(key) = parts.pop() else {
            return Err(format!("'{key}' doesn't name a key"));
        };

        Ok(Self {
            mods: parts.iter().map(|part| part.to_uppercase()).collect(),
            key: key.to_owned(),
        })
    }
}

impl TryFrom<String> for Key {
    type Error = String;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        key.parse()
    }
}

impl Key {
    /// The key as one word, for the event raisin asks Hyprland to emit when
    /// it's pressed: `shift+a`, or just `a`.
    pub(crate) fn id(&self) -> String {
        let mut id: Vec<_> = self.mods.iter().map(|m| m.to_lowercase()).collect();
        id.sort();
        id.push(self.key.to_lowercase());

        id.join("+")
    }
}

impl fmt::Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in &self.mods {
            write!(formatter, "{modifier} + ")?;
        }

        formatter.write_str(&self.key)
    }
}

/// How the switcher behaves, and how big it is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Switcher {
    /// How long Super has to stay held, in milliseconds, before the switcher
    /// appears. Long enough that a tap never puts anything on screen, short
    /// enough to feel immediate when the user does mean to look.
    #[serde(default = "default_delay")]
    pub(crate) delay: u64,
    /// How wide the window is, in pixels.
    #[serde(default = "default_width")]
    pub(crate) width: i32,
    /// How tall its list may grow, in pixels, before it starts scrolling.
    #[serde(default = "default_max_height")]
    pub(crate) max_height: i32,
    /// Whether to show each application's icon beside its name.
    #[serde(default = "enabled")]
    pub(crate) icons: bool,
}

impl Switcher {
    pub(crate) fn delay(&self) -> Duration {
        Duration::from_millis(self.delay)
    }
}

impl Default for Switcher {
    fn default() -> Self {
        Self {
            delay: default_delay(),
            width: default_width(),
            max_height: default_max_height(),
            icons: enabled(),
        }
    }
}

/// Thumbnails of the windows of the application being switched to.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Previews {
    /// Whether to capture them at all.
    #[serde(default = "enabled")]
    pub(crate) enabled: bool,
    /// How wide a thumbnail is, in pixels. Its height follows the window's
    /// own proportions.
    #[serde(default = "default_preview_width")]
    pub(crate) width: u32,
}

impl Default for Previews {
    fn default() -> Self {
        Self {
            enabled: enabled(),
            width: default_preview_width(),
        }
    }
}

fn default_cancel() -> Key {
    Key {
        mods: Vec::new(),
        key: "Escape".to_owned(),
    }
}

fn default_delay() -> u64 {
    90
}

fn default_width() -> i32 {
    460
}

fn default_max_height() -> i32 {
    420
}

fn enabled() -> bool {
    true
}

fn default_preview_width() -> u32 {
    168
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(file: &str) -> Config {
        toml::from_str(file).expect("failed to parse")
    }

    #[test]
    fn an_application_is_either_a_command_or_a_command_and_a_class() {
        let config = config(
            r#"
            [keys.apps]
            t = "ghostty"
            i = { cmd = "brave", app_id = "brave-browser" }
            "#,
        );

        assert_eq!(config.target("t"), Some(&Target::new("ghostty", None)));
        assert_eq!(
            config.target("i"),
            Some(&Target::new("brave", Some("brave-browser")))
        );
        assert_eq!(config.target("q"), None);
    }

    #[test]
    fn what_the_file_leaves_out_keeps_its_default() {
        let config = config(
            r#"
            [switcher]
            delay = 150
            "#,
        );

        assert_eq!(config.switcher.delay, 150);
        assert_eq!(config.switcher.width, default_width());
        assert_eq!(config.keys.cancel.key, "Escape");
    }

    #[test]
    fn nothing_is_bound_until_the_file_says_so() {
        assert!(Config::default().keys.apps.is_empty());
        assert!(config("").keys.apps.is_empty());
    }

    #[test]
    fn an_application_key_may_carry_modifiers_too() {
        let config = config(
            r#"
            [keys.apps]
            a = "audacity"
            "SHIFT + a" = "teams-for-linux"
            "#,
        );

        assert_eq!(config.target("a"), Some(&Target::new("audacity", None)));
        assert_eq!(
            config.target("shift+a"),
            Some(&Target::new("teams-for-linux", None))
        );
    }

    #[test]
    fn a_key_names_itself_the_same_way_however_it_was_written() {
        let spelled: Key = "SHIFT + A".to_owned().try_into().expect("a key");
        let differently: Key = "shift+a".to_owned().try_into().expect("a key");

        assert_eq!(spelled.id(), "shift+a");
        assert_eq!(spelled.id(), differently.id());
    }

    #[test]
    fn a_key_may_carry_modifiers() {
        let config = config(
            r#"
            [keys]
            next = "Tab"
            previous = "SHIFT + Tab"
            "#,
        );

        assert_eq!(
            config.keys.next,
            Some(Key {
                mods: vec![],
                key: "Tab".to_owned()
            })
        );
        assert_eq!(
            config.keys.previous,
            Some(Key {
                mods: vec!["SHIFT".to_owned()],
                key: "Tab".to_owned()
            })
        );
        assert_eq!(config.keys.previous.unwrap().to_string(), "SHIFT + Tab");
    }

    #[test]
    fn a_typo_is_an_error_rather_than_a_setting_that_quietly_does_nothing() {
        let error = toml::from_str::<Config>(
            r#"
            [switcher]
            dealy = 150
            "#,
        )
        .expect_err("unknown fields should be refused");

        assert!(error.to_string().contains("dealy"), "{error}");
    }

    #[test]
    fn previews_can_be_turned_off_without_touching_anything_else() {
        let config = config(
            r#"
            [previews]
            enabled = false
            "#,
        );

        assert!(!config.previews.enabled);
        assert_eq!(config.previews.width, default_preview_width());
        assert!(config.switcher.icons);
    }
}
