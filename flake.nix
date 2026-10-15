{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    make-shell.url = "github:nicknovitski/make-shell";
    rust-overlay.url = "github:oxalica/rust-overlay";
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix/release-0.12";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    cargo2nix-ifd = {
      url = "github:kgtkr/cargo2nix-ifd";
      inputs.cargo2nix.follows = "cargo2nix";
    };
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      imports = [
        inputs.make-shell.flakeModules.default
        ./server/default.nix
        ./wasm-engine/default.nix
        ./image.nix
      ];
      perSystem = { self', pkgs, system, ... }:
        {
          _module.args = {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [ inputs.cargo2nix.overlays.default ];
            };
            cargo2nix-ifd-lib = inputs.cargo2nix-ifd.mkLib pkgs;
          };
          packages = {
            gcsim-mcp = pkgs.runCommand "gcsim-mcp"
              {
                nativeBuildInputs = [ pkgs.makeWrapper ];
              } ''
              mkdir -p $out/bin
              makeWrapper ${self'.packages.server}/bin/gcsim-mcp-server $out/bin/gcsim-mcp \
                --set-default WASM_PATH "${self'.packages.wasm-engine}/share/wasm-engine.wasm"
            '';
            default = self'.packages.gcsim-mcp;
          };
          make-shells.default = {
            packages = [
              pkgs.nixpkgs-fmt
              pkgs.just
              pkgs.skopeo
              pkgs.manifest-tool
              pkgs.treefmt
              pkgs.dprint
              pkgs.gnused
              pkgs.gnugrep
              pkgs.gawk
            ];
          };
        };
    };
}
