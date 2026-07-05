{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    crane,
  }: let
    systems = ["aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
    perSystem = forAllSystems (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = ./.;
        buildInputs = [];
        nativeBuildInputs = with pkgs; [clang mold rustToolchain];

        commonArgs = {
          pname = "blog";
          version = "latest";
          strictDeps = true;
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          stdenv = p: p.stdenvAdapters.useMoldLinker (p.llvmPackages.stdenv);
          CARGO_PROFILE = "release";
          CARGO_BUILD_RUSTFLAGS = "-C linker=clang -C link-arg=-fuse-ld=${pkgs.mold}/bin/mold";
          inherit src buildInputs nativeBuildInputs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        bin = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
        with pkgs; {
          checks = {
            inherit bin;

            told-clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets";
              }
            );
          };

          packages = {
            inherit bin;
            default = bin;
          };

          devShells.default = mkShell {
            inputsFrom = [bin];
            buildInputs = [
              pkgs.miniserve
              pkgs.xmlformat
              pkgs.cargo-flamegraph
              pkgs.perf
              pkgs.marksman
              pkgs.harper
            ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            CARGO_BUILD_RUSTFLAGS = "-C linker=clang -C link-arg=-fuse-ld=${pkgs.mold}/bin/mold";
          };
        }
    );
  in {
    checks = nixpkgs.lib.mapAttrs (_: value: value.checks) perSystem;
    packages = nixpkgs.lib.mapAttrs (_: value: value.packages) perSystem;
    devShells = nixpkgs.lib.mapAttrs (_: value: value.devShells) perSystem;
  };
}
