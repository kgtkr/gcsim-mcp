{ lib, ... }: {
  perSystem = { config, system, pkgs, cargo2nix-ifd-lib, ... }:
    let
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      projectName = "gcsim-mcp-server";
      src = ./.;
      filteredSrc = cargo2nix-ifd-lib.filterSrc {
        inherit src;
        inherit projectName;
      };
      generatedSrc = cargo2nix-ifd-lib.generateSrc {
        src = filteredSrc;
        inherit projectName rustToolchain;
      };
      rustPkgs = pkgs.rustBuilder.makePackageSet {
        packageFun = import "${generatedSrc}/Cargo.nix";
        inherit rustToolchain;
        rustcLinkFlags = [ "--remap-path-prefix=${rustToolchain}=/rust" ];
        rustcBuildFlags = [ "--remap-path-prefix=${rustToolchain}=/rust" ];
      };
    in
    {
      packages = {
        server = rustPkgs.workspace.gcsim-mcp-server { };
      };
      make-shells.default = {
        env = {
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
        packages = [
          rustToolchain
        ];
      };
    };
}
