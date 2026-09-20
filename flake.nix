{
  description = "Run-or-raise for Hyprland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    naersk.url = "github:nix-community/naersk/master";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      naersk,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;

      # The switcher window is GTK 4 on the compositor's overlay layer.
      guiInputs =
        pkgs: with pkgs; [
          gtk4
          gtk4-layer-shell
        ];
    in
    {
      packages = forAllSystems (system: {
        default =
          let
            pkgs = import nixpkgs { inherit system; };
            naersk-lib = pkgs.callPackage naersk { };
          in
          naersk-lib.buildPackage {
            src = ./.;
            nativeBuildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
            ];
            buildInputs = guiInputs pkgs;
          };
      });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/raisin";
        };
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs =
              (with pkgs; [
                cargo
                rustc
                rustfmt
                pre-commit
                rustPackages.clippy
              ])
              ++ guiInputs pkgs;
            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
        }
      );
    };
}
