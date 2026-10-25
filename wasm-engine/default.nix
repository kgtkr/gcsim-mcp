{ ... }: {
  perSystem = { pkgs, ... }:
    let
      wasm-engine = pkgs.buildGoModule.override { go = pkgs.go_latest; } {
        pname = "wasm-engine";
        version = "0.1.0";
        src = ./.;

        vendorHash = "sha256-cHX5lTOusCOdc+DNNL2P1G/XMVLPc+SVtlsbz+y3BbQ=";

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
