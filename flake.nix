{
  description = "A flake for Rust development";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, ... }@inputs:
    {
      nixosModules = {
        wyattwtf = import ./service.nix { inherit self; };
      };
    }
    // inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import inputs.rust-overlay) ];
        };

        nativeBuildInputs = with pkgs; [
          cargo-leptos
          binaryen
          wasm-bindgen-cli
          dart-sass
        ];

        rustBin = with pkgs; [
          (rust-bin.stable.latest.default.override {
            extensions = [
              "clippy"
              "rust-src"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          })
        ];
      in
      {
        devShells =
          let
            rustShell = pkgs.mkShell {
              name = "rust-development-shell";
              nativeBuildInputs =
                rustBin
                ++ nativeBuildInputs
                ++ (with pkgs; [
                  gcc
                  rust-analyzer
                ]);
            };
          in
          {
            rust = rustShell;
            default = rustShell;
          };

        packages = {
          default =
            let
              cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
              leptosApp = pkgs.rustPlatform.buildRustPackage {
                pname = cargoToml.package.name;
                inherit (cargoToml.package) version;
                src = ./.;

                cargoLock = {
                  lockFile = ./Cargo.lock;
                };

                nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [ lld ]);

                buildPhase = ''
                  export HOME=$(mktemp -d)
                  cargo leptos build --release
                '';

                installPhase = ''
                  mkdir -p $out/bin $out/share
                  cp target/release/${cargoToml.package.name} $out/bin/
                  cp -r target/site $out/share/
                '';

                doCheck = false;
              };
            in
            pkgs.writeShellScriptBin "${cargoToml.package.name}-${cargoToml.package.version}" ''
              export LEPTOS_SITE_ROOT=${leptosApp}/share/site
              exec ${leptosApp}/bin/${cargoToml.package.name} "$@"
            '';
        };

        checks =
          let
            mkCheck =
              {
                name,
                cmds,
                src ? self,
                inputs ? [ ],
              }:
              pkgs.runCommand name { buildInputs = inputs; } ''
                cd ${src}
                ${pkgs.lib.strings.concatLines cmds}
                touch $out
              '';

            checkArgs = {
              rustFormatting = {
                inputs = rustBin;
                cmds = [ "cargo fmt --check" ];
              };
            };
          in
          builtins.mapAttrs (name: args: mkCheck (args // { inherit name; })) checkArgs;
      }
    );
}
