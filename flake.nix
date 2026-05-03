{
  description = "wyatt.wtf development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.treefmt-nix.flakeModule ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      flake.nixosModules = {
        wyattwtf = import ./service.nix { inherit (inputs) self; };
      };

      perSystem =
        { self', system, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };

          cargoToml = fromTOML (builtins.readFile ./Cargo.toml);

          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "clippy"
              "rust-src"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          };

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          nativeBuildInputs = with pkgs; [
            cargo-leptos
            binaryen
            wasm-bindgen-cli
            dart-sass
          ];

          mkCargoCheck =
            name: command:
            rustPlatform.buildRustPackage {
              pname = name;
              inherit (cargoToml.package) version;
              src = ./.;

              cargoLock = {
                lockFile = ./Cargo.lock;
              };

              nativeBuildInputs = nativeBuildInputs ++ [ rustToolchain ];

              buildPhase = ''
                runHook preBuild
                export HOME=$(mktemp -d)
                export CARGO_TARGET_DIR=$(mktemp -d)
                ${command}
                runHook postBuild
              '';

              installPhase = ''
                touch $out
              '';

              doCheck = false;
            };

          mkNixCheck =
            name: package: command:
            pkgs.runCommand name { nativeBuildInputs = [ package ]; } ''
              cd ${./.}
              ${command}
              touch $out
            '';

          leptosApp = rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            inherit (cargoToml.package) version;
            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [ lld ]);

            buildPhase = ''
              export HOME=$(mktemp -d)
              cargo leptos build --release --precompress
            '';

            installPhase = ''
              mkdir -p $out/bin $out/share
              cp target/release/${cargoToml.package.name} $out/bin/
              cp -r target/site $out/share/
            '';

            doCheck = false;
          };
        in
        {
          _module.args.pkgs = pkgs;

          treefmt.config = {
            projectRootFile = "flake.nix";
            programs = {
              leptosfmt.enable = true;
              nixfmt.enable = true;
              prettier = {
                enable = true;
                includes = [
                  "*.scss"
                  "*.yaml"
                  "*.yml"
                ];
              };
            };
          };

          devShells =
            let
              rustShell = pkgs.mkShell {
                name = "rust-development-shell";
                nativeBuildInputs = [
                  rustToolchain
                ]
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

          packages.default = pkgs.writeShellScriptBin "${cargoToml.package.name}-${cargoToml.package.version}" ''
            export LEPTOS_SITE_ROOT=${leptosApp}/share/site
            exec ${leptosApp}/bin/${cargoToml.package.name} "$@"
          '';

          checks = {
            statix = mkNixCheck "statix" pkgs.statix ''
              statix check .
            '';

            deadnix = mkNixCheck "deadnix" pkgs.deadnix ''
              deadnix --fail .
            '';

            cargo-fmt = pkgs.runCommand "cargo-fmt" { buildInputs = [ rustToolchain ]; } ''
              cd ${./.}
              cargo fmt --check
              touch $out
            '';

            cargo-clippy = mkCargoCheck "cargo-clippy" ''
              cargo clippy --workspace --all-targets --locked --offline -- -D warnings
            '';

            cargo-test = mkCargoCheck "cargo-test" ''
              cargo test --workspace --all-targets --locked --offline
            '';

            hydrate-check = mkCargoCheck "hydrate-check" ''
              cargo check --lib --no-default-features --features hydrate --target wasm32-unknown-unknown --locked --offline
            '';

            leptos-build = self'.packages.default;
          };
        };
    };
}
