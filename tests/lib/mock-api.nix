# tests/lib/mock-api.nix — Nix wrapper for the stateful mock API server.
#
# Usage in a NixOS VM test:
#
#   let
#     mockApi = import ./lib/mock-api.nix { inherit pkgs; };
#   in
#   pkgs.testers.nixosTest {
#     nodes.controlplane = {
#       imports = [ (mockApi.module { port = 3000; }) ];
#     };
#     ...
#   };

{ pkgs }:

let
  mockApiScript = ./mock-api.py;
in
{
  # Standalone package — run with: hearth-mock-api --port 3000
  package = pkgs.writeShellScriptBin "hearth-mock-api" ''
    exec ${pkgs.python3}/bin/python3 ${mockApiScript} "$@"
  '';

  # NixOS module fragment for use in test nodes.
  # Usage: imports = [ (mockApi.module { port = 3000; }) ];
  module = { port ? 3000 }: { config, ... }: {
    systemd.services.hearth-mock-api = {
      description = "Hearth Mock API Server";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${pkgs.writeShellScriptBin "hearth-mock-api" ''
          exec ${pkgs.python3}/bin/python3 ${mockApiScript} --port ${toString port}
        ''}/bin/hearth-mock-api";
        Restart = "on-failure";
      };
    };
    networking.firewall.allowedTCPPorts = [ port ];

    # Test scripts use `curl | python3` to introspect mock API state
    environment.systemPackages = [ pkgs.python3 ];
  };
}
