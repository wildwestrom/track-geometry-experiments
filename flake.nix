{
  description = "Rust example flake for Zero to Nix";

  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    {
      nixpkgs,
      systems,
      rust-overlay,
      ...
    }:
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
          default = pkgs.mkShell {
            packages = with pkgs; [
              rust-toolchain
              wgsl-analyzer
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
              cargo-machete
              cargo-unused-features
              cargo-tarpaulin
              tracy-wayland
              valgrind-light
              heaptrack
            ];

            # Set up environment variables for Bevy/Wayland
            LD_LIBRARY_PATH =
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
          };
        }
      );
    };
}
