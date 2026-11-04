{ ... }: {
  perSystem = { pkgs, ... }:
    let
      wasm-engine = pkgs.buildGoModule.override { go = pkgs.go_latest; } {
        pname = "wasm-engine";
        version = "0.1.0";
        src = ./.;

        vendorHash = "sha256-RZqG4tUDT14BitD1t+/cz1ZK4Rj4W2N0gW9VispajnA=";

        buildPhase = ''
          runHook preBuild
          export GOOS=wasip1
          export GOARCH=wasm
          export CGO_ENABLED=0
          go build -trimpath -ldflags="-s -w" -o wasm-engine.wasm .
          runHook postBuild
        '';

        installPhase = ''
          runHook preInstall
          mkdir -p $out/share
          install -m 644 wasm-engine.wasm $out/share/wasm-engine.wasm
          runHook postInstall
        '';
      };
    in
    {
      packages = {
        inherit wasm-engine;
      };

      make-shells.default = {
        packages = [
          pkgs.go_latest
        ];
      };
    };
}
