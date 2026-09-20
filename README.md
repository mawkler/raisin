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

It binds `Super` + a letter for every application in the table at the top of `src/bindings.rs`, so
adding another letter is a line of Rust and a restart — no compositor configuration to edit.

- Tap `Super` + the letter and the window is focused immediately, with nothing on screen.
- Keep `Super` held a moment longer and the switcher appears: every open window, grouped by
  application, with the one you'd get highlighted.
- Press the same letter again to cycle through that application's windows, press another mapped
  letter to switch to that application instead, release `Super` to confirm, or press `Esc` to
  cancel.

While the switcher runs it takes those letters over from your own configuration; `hyprctl reload`
gives them back. `Super` and `Escape` are shared rather than taken over: a binding of your own on
`Super` — a launcher on tap, say — keeps working, and Hyprland shadows it by itself while you're
holding `Super` to pick a window.

`raisin switch <app>` does the same thing from a keybinding of your own, and says so plainly if the
switcher isn't running.

## Run/install

### Run with Nix

`nix run github:mawkler/raisin -- <app>`

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
