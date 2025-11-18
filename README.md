# Rasin

Run-or-raise for [Niri](https://github.com/YaLTeR/niri).

## Run/install

### Run with Nix

`nix run github:mawkler/maconomy-cli -- <program_name>`

### Install with cargo

`cargo install github:mawkler/maconomy-cli -- <program_name>`

## Usage

Output of `raisin --help`

```help
Run-or-raise for Niri

Usage: raisin <APP_CLASS> [APP_CMD]

Arguments:
  <APP_CLASS>
          Application's app_id (e.g., `com.mitchellh.ghostty`).

          Will do partial matching.

  [APP_CMD]
          Command to run the application (e.g., `ghostty`). Optional.

          If omitted, use `app_class`.

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  raisin ghostty
  raisin brave-browser brave
```
