# Raisin 🍇

*Run-or-raise*:

- If the program isn't running: launch it
- If the program is running: jump to one of its windows (and cycle through them if there's more than one)

Intended to be called from a compositor keybinding like so:

- `super + t`: terminal
- `super + w`: web browser
- `super + s`: spotify
- etc...

Currently supports [Niri](https://github.com/YaLTeR/niri) and [Hyprland](https://hyprland.org), but has a small integration layer for adding support for more compositors in the future.

## The switcher

On Hyprland, raisin can show an Alt-Tab style overlay while you hold `Super`. Start it once, for
example from your Hyprland startup configuration:

```
exec-once = raisin daemon
```

It binds `Super` + a letter for every application in `[keys.apps]`, so adding another letter is a
line of configuration — no compositor configuration to edit, and no restart.

- Tap `Super` + the letter and the window is focused immediately, with nothing on screen.
- Keep `Super` held a moment longer and the switcher appears: every open window, grouped by
  application, with the one you'd get highlighted. The windows you're choosing between show what
  they currently look like.
- Press the same letter again to cycle through that application's windows, press another mapped
  letter to switch to that application instead, release `Super` to confirm, or press `Esc` to
  cancel.

While the switcher runs it takes those letters over from your own configuration; `hyprctl reload`
gives them back. `Super` and `Escape` are shared rather than taken over: a binding of your own on
`Super` — a launcher on tap, say — keeps working, and Hyprland shadows it by itself while you're
holding `Super` to pick a window.

`raisin switch <app>` does the same thing from a keybinding of your own, and says so plainly if the
switcher isn't running.

## Configuration

Raisin reads `$XDG_CONFIG_HOME/raisin/config.toml` — usually `~/.config/raisin/config.toml` — or
whichever file `raisin daemon --config <path>` names. Every setting has a default, so the file is
optional, and only needs the parts you want to change.

Raisin binds nothing until the file says so: without `[keys.apps]` the switcher runs but no key
does anything.

```toml
[keys]
# These only do something while the switcher is on screen. The rest of the time
# they belong to whatever you're using.
next = "Tab"              # move the highlight on
previous = "SHIFT + Tab"  # and back
cancel = "Escape"         # close without switching

# Which letter targets which application, held with Super.
[keys.apps]
t = "ghostty"                                        # Super + t
s = "spotify"
i = { cmd = "brave", app_id = "brave-browser" }      # when the window class differs

[switcher]
delay = 90          # milliseconds Super has to stay held before the switcher appears
width = "60%"       # of the screen, or a number of pixels like 900; it scrolls past this
max_height = "40%"
icons = true        # show each application's icon beside its name

[previews]
enabled = true    # show what each window looks like
height = 105      # pixels tall; every thumbnail is, and the width follows the window

# What to call an application, by the window class it uses. Optional: without
# an entry a group is named after the title its windows opened under.
[names]
brave-browser = "Brave"
"com.mitchellh.ghostty" = "Ghostty"
```

A group is named after the title its windows carried when they opened, which is usually the
application's own name — `Ghostty` rather than `com.mitchellh.ghostty`. Applications that open
onto something else are worth naming yourself: Brave's windows start out called `New Tab - Brave`.
Use `[names]` for those; the key is the window class, which is the part that doesn't change as you
use the application.

You hold `Super` throughout a switch, so it's implied: `next = "Tab"` means Super and Tab.

Holding `Ctrl` with an application's key runs its `cmd` again, for when the window you want
doesn't exist yet: `Super + Ctrl + t` starts another terminal rather than switching to one. If the
switcher is open it closes, without focusing anything: asking for a new window has answered the
question it was asking.

Holding `Shift` with an application's key walks its windows the other way, the way Alt-Shift-Tab
does. That needs no configuration — and it's why a key that names `Shift` itself doesn't get one:
`"SHIFT + a"` is free only while plain `a` is unmapped, and raisin says so if you map both.

Saving the file is enough: raisin watches it and rebinds straight away, ending any switch that was
in progress under the old keys. Raisin says so on startup when one of its keys is already bound in your Hyprland configuration, and
when two of its own keys are the same key.

A setting the file misspells is an error — at startup it stops the
daemon, and on a reload it leaves the running configuration alone and says what was wrong, since a
half-saved file is a normal thing for an editor to leave behind for a moment.

## Run/install

### Run with Nix

`nix run github:mawkler/raisin -- <app>`

### NixOS

The flake exports a module:

```nix
{
  inputs.raisin.url = "github:mawkler/raisin";

  # ...then, in your configuration:
  imports = [ inputs.raisin.nixosModules.default ];

  services.raisin = {
    enable = true;
    settings.keys.apps = {
      t = "ghostty";
      i = {
        cmd = "brave";
        app_id = "brave-browser";
      };
    };
  };
}
```

The daemon runs as a user service started by `graphical-session.target`, so the session has to reach
systemd: `programs.hyprland.withUWSM = true` does that. Leaving `settings` out keeps your own
`~/.config/raisin/config.toml`, which you can edit without rebuilding.

### Install with cargo

`cargo install --git github:mawkler/raisin`

## Usage

```help
Run-or-raise for Hyprland and Niri

Usage: raisin <APP> [APP_ID]
       raisin [APP] [APP_ID] <COMMAND>

Commands:
  daemon  Run the switcher. Hyprland only
  switch  Switch to an application through the running switcher
  help    Print this message or the help of the given subcommand(s)

Arguments:
  <APP>
          Command to run the application (e.g., `ghostty`)

  [APP_ID]
          Window app_id to match (e.g., `com.mitchellh.ghostty`). Optional.

          If omitted, the app name is used as a substring to match against window class names.

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  raisin ghostty
  raisin ghostty com.mitchellh.ghostty
  raisin daemon
```
