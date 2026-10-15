{ lib, ... }: {
  perSystem = { self', pkgs, ... }:
    let
      args = {
        name = "gcsim-mcp";
        tag = "latest";
        contents = [
          pkgs.cacert
        ];
        fakeRootCommands = ''
          mkdir -p tmp
          chmod 1777 tmp
        '';
        config = {
          Entrypoint = [ (lib.getExe self'.packages.gcsim-mcp) ];
          Env = [
            "GCSIM_HOST=0.0.0.0"
            "GCSIM_PORT=3485"
            "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          ];
          ExposedPorts = {
            "3485/tcp" = { };
          };
          WorkingDir = "/";
        };
      };
    in
    {
      packages = {
        image = pkgs.dockerTools.buildLayeredImage args;
        streamImage = pkgs.dockerTools.streamLayeredImage args;
      };
    };
}
