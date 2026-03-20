# tests/lib/headscale-test.nix — Shared Headscale test infrastructure.
#
# Provides a reusable NixOS module for running a real Headscale coordination
# server in VM integration tests, plus a bootstrap script that creates a
# user and pre-auth keys.
#
# Usage in a NixOS VM test:
#
#   let
#     headscaleTest = import ./lib/headscale-test.nix { inherit pkgs; };
#   in
#   pkgs.testers.nixosTest {
#     nodes.headscale = {
#       imports = [ (headscaleTest.module {}) ];
#     };
#     nodes.client = {
#       imports = [ ../modules/headscale-client.nix ];
#       services.hearth.headscaleClient = {
#         enable = true;
#         serverUrl = "http://headscale:8080";
#       };
#       # Write a pre-auth key file for the join service
#     };
#     testScript = ''
#       headscale.wait_for_unit("headscale-bootstrap.service")
#       # Read pre-auth keys from /tmp/headscale-preauth-keys
#     '';
#   };

{ pkgs }:

let
  # Bootstrap script: creates the "hearth" user and generates pre-auth keys.
  # Writes keys to /tmp/headscale-preauth-keys (one per line) and touches
  # /tmp/headscale-bootstrap-done when complete.
  bootstrapScript = pkgs.writeShellScript "headscale-bootstrap" ''
    set -euo pipefail

    echo "[headscale-bootstrap] Waiting for Headscale to be ready..."
    for i in $(seq 1 60); do
      if ${pkgs.headscale}/bin/headscale health 2>/dev/null; then
        break
      fi
      sleep 1
    done

    echo "[headscale-bootstrap] Creating user 'hearth'..."
    ${pkgs.headscale}/bin/headscale users create hearth 2>/dev/null || true

    # Get the user ID (v0.27+ uses numeric IDs for --user flag)
    USER_ID=$(${pkgs.headscale}/bin/headscale users list -o json | ${pkgs.jq}/bin/jq -r '.[] | select(.name == "hearth") | .id')
    echo "[headscale-bootstrap] User 'hearth' has ID: $USER_ID"

    echo "[headscale-bootstrap] Generating pre-auth keys..."
    KEY1=$(${pkgs.headscale}/bin/headscale preauthkeys create --user "$USER_ID" -e 1h -o json | ${pkgs.jq}/bin/jq -r '.key')
    KEY2=$(${pkgs.headscale}/bin/headscale preauthkeys create --user "$USER_ID" -e 1h -o json | ${pkgs.jq}/bin/jq -r '.key')
    KEY3=$(${pkgs.headscale}/bin/headscale preauthkeys create --user "$USER_ID" -e 1h -o json | ${pkgs.jq}/bin/jq -r '.key')

    echo "$KEY1" > /tmp/headscale-preauth-key-1
    echo "$KEY2" > /tmp/headscale-preauth-key-2
    echo "$KEY3" > /tmp/headscale-preauth-key-3

    touch /tmp/headscale-bootstrap-done
    echo "[headscale-bootstrap] Done! Keys written to /tmp/headscale-preauth-key-{1,2,3}"
  '';
in
{
  # NixOS module for a Headscale server test node with bootstrap.
  # Usage: imports = [ (headscaleTest.module {}) ];
  module = { port ? 8080 }: { config, ... }: {
    services.headscale = {
      enable = true;
      port = port;
      address = "0.0.0.0";
      settings = {
        server_url = "http://headscale:${toString port}";
        dns = {
          base_domain = "hearth.local";
          magic_dns = true;
          nameservers.global = [ "127.0.0.1" ];
        };
        prefixes = {
          v4 = "100.64.0.0/10";
          v6 = "fd7a:115c:a1e0::/48";
        };
        derp = {
          urls = [ ]; # No external DERP — isolated test network
          server = {
            enabled = true;
            region_id = 999;
            stun_listen_addr = "0.0.0.0:3478";
          };
        };
      };
    };

    networking.firewall.allowedTCPPorts = [ port ];
    networking.firewall.allowedUDPPorts = [ 3478 ];

    systemd.services.headscale-bootstrap = {
      description = "Bootstrap Headscale with Hearth test data";
      after = [ "headscale.service" ];
      wants = [ "headscale.service" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = bootstrapScript;
      };
    };

    virtualisation.memorySize = 512;
  };
}
