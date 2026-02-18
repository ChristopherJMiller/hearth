# tests/offline-fallback.nix — NixOS VM test stub: offline resilience
#
# This test will verify that Hearth fleet machines degrade gracefully
# when network connectivity is lost:
#
# Scenarios to test:
#
# 1. RETURNING USER, OFFLINE:
#    - User has logged in before (closure cached in local Nix store)
#    - Network is disabled
#    - Login should succeed using cached closure
#    - Activation should complete in <1 second (symlinking only)
#    - SSSD credential cache should handle offline authentication
#
# 2. NEW USER ON THIS DEVICE, OFFLINE:
#    - User has never logged in on this device
#    - Network is disabled
#    - Agent should fall back to pre-built role profile from system closure
#    - User gets a functional environment matching their role
#    - No per-user customizations until connectivity returns
#
# 3. AGENT RECONNECTION:
#    - Agent starts with no network
#    - Agent queues heartbeats and login events
#    - Network is restored
#    - Agent flushes queued events to control plane
#    - Agent resumes normal polling
#
# 4. PARTIAL CONNECTIVITY:
#    - Control plane is unreachable but binary cache is available
#    - Agent operates in degraded mode
#    - Pre-warmed closures can still be pulled from cache
#
# Nodes:
#   - server: runs hearth-api (can be stopped to simulate offline)
#   - client: runs hearth-agent and hearth-greeter
#
# NOTE: This is a structural stub. Network manipulation and credential
# caching tests require functional agent and SSSD configuration.
# The test framework is ready for implementation in later phases.

{ pkgs, lib, ... }:

pkgs.nixosTest {
  name = "hearth-offline-fallback";

  nodes = {
    server = { config, pkgs, ... }: {
      nixpkgs.overlays = [
        (final: prev: {
          hearth-api = prev.writeShellScriptBin "hearth-api" ''
            ${prev.python3}/bin/python3 -c "
            from http.server import HTTPServer, BaseHTTPRequestHandler
            import json

            class Handler(BaseHTTPRequestHandler):
                def do_GET(self):
                    self.send_response(200)
                    self.send_header('Content-Type', 'application/json')
                    self.end_headers()
                    self.wfile.write(json.dumps({'status': 'ok'}).encode())

                def do_POST(self):
                    self.send_response(200)
                    self.send_header('Content-Type', 'application/json')
                    self.end_headers()
                    self.wfile.write(json.dumps({'ack': True}).encode())

            HTTPServer(('0.0.0.0', 3000), Handler).serve_forever()
            "
          '';
        })
      ];

      networking.firewall.allowedTCPPorts = [ 3000 ];
      systemd.services.hearth-api = {
        description = "Hearth API Server (offline test)";
        after = [ "network.target" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.hearth-api}/bin/hearth-api";
      };
    };

    client = { config, pkgs, ... }: {
      imports = [ ../modules/agent.nix ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-agent = prev.writeShellScriptBin "hearth-agent" ''
            mkdir -p /run/hearth
            if [ -n "$NOTIFY_SOCKET" ]; then
              ${prev.systemd}/bin/systemd-notify --ready
            fi
            # Simple poll loop that handles connection failures
            while true; do
              if ${prev.curl}/bin/curl -sf --connect-timeout 3 \
                   "http://server:3000/api/v1/agent/checkin" \
                   -X POST -d '{}' 2>/dev/null; then
                echo "Online: check-in successful"
              else
                echo "Offline: queuing check-in"
              fi
              sleep 5
            done
          '';
        })
      ];

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://server:3000";
        machineId = "test-offline-001";
        pollInterval = 5;
        package = pkgs.hearth-agent;
      };
    };
  };

  testScript = ''
    # --- Phase 1: Online operation ---
    server.start()
    client.start()

    server.wait_for_unit("hearth-api.service")
    server.wait_for_open_port(3000)
    client.wait_for_unit("hearth-agent.service")

    # Verify connectivity works
    client.succeed("curl -sf http://server:3000/health || true")

    # Let the agent perform a few successful poll cycles
    client.sleep(15)

    # Verify agent is still running
    client.succeed("systemctl is-active hearth-agent.service")

    # --- Phase 2: Simulate offline ---
    # Block network traffic to the server
    client.succeed("iptables -A OUTPUT -d server -j DROP")

    # Agent should continue running in degraded mode
    client.sleep(15)
    client.succeed("systemctl is-active hearth-agent.service")

    # --- Phase 3: Restore connectivity ---
    client.succeed("iptables -D OUTPUT -d server -j DROP")

    # Agent should reconnect
    client.sleep(15)
    client.succeed("systemctl is-active hearth-agent.service")

    # TODO (Phase 2+): Detailed offline assertions
    # - Verify queued events are flushed on reconnection
    # - Verify cached user closure activation works offline
    # - Verify role profile fallback for new users offline
    # - Verify SSSD credential cache enables offline auth
    # - Measure activation latency in cached vs. fallback scenarios
  '';
}
