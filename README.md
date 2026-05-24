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

## Run/install

### Run with Nix

`nix run github:mawkler/raisin -- <app>`

### Install with cargo

`cargo install --git github:mawkler/raisin`

## Usage

```help
Run-or-raise for Niri and Hyprland

Usage: raisin <APP> [APP_ID]

Arguments:
  <APP>
          Command to run the application (e.g., `ghostty`).

  [APP_ID]
          Window app_id to match (e.g., `com.mitchellh.ghostty`). Optional.

          If omitted, the app name is used as a substring to match against
          window class names.

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  raisin ghostty
  raisin ghostty com.mitchellh.ghostty
```
