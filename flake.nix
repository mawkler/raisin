{
  description = "Run-or-raise for Niri and Hyprland";

  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      utils,
      naersk,
      ...
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };

        mkRaisin = features: naersk-lib.buildPackage {
          src = ./.;
          cargoBuildOptions = x: x ++ [ "--no-default-features" "--features" features ];
        };

        packages = {
          hyprland = mkRaisin "hyprland";
          niri = mkRaisin "niri";
        };
      in
      {
        inherit packages;

        defaultPackage = packages.hyprland;

        devShell =
          with pkgs;
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              pre-commit
              rustPackages.clippy
            ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      }
    );
}
