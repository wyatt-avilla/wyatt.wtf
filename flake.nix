{
  description = "A flake for Rust development";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, ... }@inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import inputs.rust-overlay) ];
        };

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
                ++ (with pkgs; [
                  gcc
                  rust-analyzer
                  cargo-leptos
                  wasm-bindgen-cli
                  sass
                ]);
            };
          in
          {
            rust = rustShell;
            default = rustShell;
          };

        packages.default =
          let
            cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          in
          pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            inherit (cargoToml.package) version;

            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
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
