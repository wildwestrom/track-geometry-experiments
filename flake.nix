{
  description = "Rust example flake for Zero to Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    devenv.url = "github:cachix/devenv";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    {
      self,
      nixpkgs,
      devenv,
      systems,
      rust-overlay,
      ...
    }@inputs:
    let
      forEachSystem = nixpkgs.lib.genAttrs (import systems);
    in
    {
      devShells = forEachSystem (
        system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = nixpkgs.legacyPackages.${system}.extend (
            final: prev: {
              rustPkgs = import nixpkgs {
                inherit system overlays;
              };
            }
          );
          rust-toolchain = pkgs.rustPkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              {
                packages = with pkgs; [
                  rust-toolchain
                  stdenv.cc.cc.lib
                  mold
                  clang
                  pkg-config
                  cargo-binutils
                  just

                  libGL
                  alsa-lib
                  libudev-zero
                  vulkan-loader
                  wayland
                  libxkbcommon

                  cargo-watch
                  cargo-flamegraph
                  tracy-wayland
                  valgrind-light
                  heaptrack
                ];

                # Set up environment variables for Bevy/Wayland
                env.LD_LIBRARY_PATH =
                  with pkgs;
                  lib.makeLibraryPath [
                    libxkbcommon
                    libGL
                    vulkan-loader
                    wayland
                    alsa-lib
                    libudev-zero
                    stdenv.cc.cc.lib
                  ];
              }
            ];
          };
        }
      );
    };
}
