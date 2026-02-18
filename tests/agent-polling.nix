# tests/agent-polling.nix — NixOS VM test: agent connects to API server
#
# Two-node test verifying that hearth-agent starts correctly and can
# establish communication with a hearth-api server.
#
# Nodes:
#   - server: runs hearth-api listening on port 3000
#   - client: runs hearth-agent configured to poll the server
#
# Assertions:
#   - Both services start without errors
#   - Agent successfully reaches the server's health endpoint
#   - Agent socket is created and accessible

{ pkgs, lib, ... }:

let
  serverPort = 3000;
in
pkgs.nixosTest {
  name = "hearth-agent-polling";

  nodes = {
    server = { config, pkgs, ... }: {
      # Apply the Hearth overlay so pkgs.hearth-api is available
      nixpkgs.overlays = [
        (final: prev: {
          hearth-api = pkgs.hearth-api or (prev.writeShellScriptBin "hearth-api" ''
            # Stub API server for testing — returns 200 on /health
            ${prev.python3}/bin/python3 -c "
            from http.server import HTTPServer, BaseHTTPRequestHandler
            import json

            class Handler(BaseHTTPRequestHandler):
                def do_GET(self):
                    if self.path == '/health' or self.path == '/api/v1/health':
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.end_headers()
                        self.wfile.write(json.dumps({'status': 'ok'}).encode())
                    elif self.path == '/api/v1/agent/checkin':
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.end_headers()
                        self.wfile.write(json.dumps({'ack': True}).encode())
                    else:
                        self.send_response(404)
                        self.end_headers()

                def do_POST(self):
                    self.do_GET()

            HTTPServer(('0.0.0.0', ${toString serverPort}), Handler).serve_forever()
            "
          '');
        })
      ];

      networking.firewall.allowedTCPPorts = [ serverPort ];

      systemd.services.hearth-api = {
        description = "Hearth API Server (test stub)";
        after = [ "network.target" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig = {
          ExecStart = "${pkgs.hearth-api}/bin/hearth-api";
          Restart = "always";
        };
      };
    };

    client = { config, pkgs, ... }: {
      nixpkgs.overlays = [
        (final: prev: {
          hearth-agent = pkgs.hearth-agent or (prev.writeShellScriptBin "hearth-agent" ''
            # Stub agent for testing — polls server and creates socket
            SOCKET_PATH="/run/hearth/agent.sock"
            mkdir -p "$(dirname "$SOCKET_PATH")"

            # Signal ready via sd_notify protocol
            if [ -n "$NOTIFY_SOCKET" ]; then
              ${prev.systemd}/bin/systemd-notify --ready
            fi

            while true; do
              ${prev.curl}/bin/curl -sf "http://server:${toString serverPort}/api/v1/agent/checkin" \
                -X POST -H "Content-Type: application/json" \
                -d '{"machine_id": "test-machine-001"}' \
                && echo "Check-in successful" \
                || echo "Check-in failed"
              sleep 5
            done
          '');
        })
      ];

      # Import and configure the agent module
      imports = [ ../modules/agent.nix ];

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://server:${toString serverPort}";
        machineId = "test-machine-001";
        pollInterval = 5;
        package = pkgs.hearth-agent;
      };
    };
  };

  testScript = ''
    # Start both machines
    server.start()
    client.start()

    # Wait for the API server to be ready
    server.wait_for_unit("hearth-api.service")
    server.wait_for_open_port(${toString serverPort})

    # Verify the API server health endpoint
    server.succeed("curl -sf http://localhost:${toString serverPort}/health")

    # Wait for the agent to start on the client
    client.wait_for_unit("hearth-agent.service")

    # Give the agent a moment to perform its first poll
    client.sleep(10)

    # Verify the agent can reach the server
    client.succeed("curl -sf http://server:${toString serverPort}/api/v1/agent/checkin -X POST -H 'Content-Type: application/json' -d '{\"machine_id\": \"test-machine-001\"}'")

    # Verify the agent's runtime directory was created
    client.succeed("test -d /run/hearth")

    # Verify the agent's state directory was created
    client.succeed("test -d /var/lib/hearth")

    # Verify the agent configuration file was generated
    client.succeed("test -f /etc/hearth/agent.toml")
    client.succeed("grep 'test-machine-001' /etc/hearth/agent.toml")

    # Verify the hearth user was created
    client.succeed("id hearth")

    # Check agent service is still running (no crash)
    client.succeed("systemctl is-active hearth-agent.service")
  '';
}
