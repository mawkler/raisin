//! Which letter targets which application.
//!
//! The daemon installs a `Super` + letter binding for every entry when it
//! starts, so extending the switcher to another letter means adding a line
//! here — no compositor configuration involved.

/// An application the switcher can target.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// `Super` + [`key`](Self::key) switches to [`app`](Self::app).
pub(crate) struct Binding {
    pub(crate) key: char,
    pub(crate) app: &'static str,
    pub(crate) app_id: Option<&'static str>,
}

impl Binding {
    pub(crate) fn target(&self) -> Target {
        Target::new(self.app, self.app_id)
    }
}

/// Every letter the switcher answers to. Add a line to map another one.
pub(crate) const BINDINGS: &[Binding] = &[
    Binding {
        key: 'i',
        app: "brave",
        app_id: Some("brave-browser"),
    },
    Binding {
        key: 's',
        app: "spotify",
        app_id: None,
    },
    Binding {
        key: 't',
        app: "ghostty",
        app_id: Some("com.mitchellh.ghostty"),
    },
    // The browser answers to both letters: `i` the way the specification
    // spells it, `w` the way the README does.
    Binding {
        key: 'w',
        app: "brave",
        app_id: Some("brave-browser"),
    },
];

/// The application `Super` + `key` targets, if any.
pub(crate) fn binding(key: char) -> Option<&'static Binding> {
    BINDINGS.iter().find(|binding| binding.key == key)
}
