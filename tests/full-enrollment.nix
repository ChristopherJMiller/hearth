# tests/full-enrollment.nix — NixOS VM test stub: full enrollment flow
#
# This test will verify the complete device enrollment lifecycle:
#
# 1. An enrollment image boots with hearth-enrollment running
# 2. The enrollment TUI detects (mock) hardware
# 3. The TUI contacts the control plane enrollment endpoint
# 4. An enrollment code is displayed
# 5. The control plane "approves" the enrollment
# 6. The enrollment agent partitions a (virtual) disk
# 7. A NixOS system closure is "installed" (from a mock cache)
# 8. The system reboots into the installed configuration
# 9. hearth-agent starts and checks in with the control plane
#
# Nodes:
#   - controlplane: runs hearth-api with enrollment endpoints
#   - device: boots the enrollment image, then reboots into installed system
#
# NOTE: This is a structural stub. The test body will be filled in during
# later implementation phases when the enrollment binary and API endpoints
# are functional. The test framework and node definitions are ready to use.

{ pkgs, lib, ... }:

pkgs.testers.nixosTest {
  name = "hearth-full-enrollment";

  nodes = {
    controlplane = { config, pkgs, ... }: {
      nixpkgs.overlays = [
        (final: prev: {
          hearth-api = prev.writeShellScriptBin "hearth-api" ''
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
                    elif '/enroll' in self.path and '/status' in self.path:
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.end_headers()
                        self.wfile.write(json.dumps({
                            'enrollment_id': 'test-enroll-001',
                            'status': 'pending_approval'
                        }).encode())
                    else:
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.end_headers()
                        self.wfile.write(json.dumps({'status': 'ok'}).encode())

                def do_POST(self):
                    if '/enroll' in self.path:
                        self.send_response(200)
                        self.send_header('Content-Type', 'application/json')
                        self.end_headers()
                        self.wfile.write(json.dumps({
                            'enrollment_id': 'test-enroll-001',
                            'enrollment_code': 'ABC-123',
                            'status': 'pending'
                        }).encode())
                    else:
                        self.send_response(200)
                        self.end_headers()

            HTTPServer(('0.0.0.0', 3000), Handler).serve_forever()
            "
          '';
        })
      ];

      networking.firewall.allowedTCPPorts = [ 3000 ];

      systemd.services.hearth-api = {
        description = "Hearth API Server (enrollment test)";
        after = [ "network.target" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.hearth-api}/bin/hearth-api";
      };
    };

    device = { config, pkgs, ... }: {
      # TODO: In later phases, this node will boot the enrollment image
      # and run through the full enrollment flow.
      # For now, we verify the basic enrollment module loads.
      imports = [ ../modules/enrollment.nix ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-enrollment = prev.writeShellScriptBin "hearth-enrollment" ''
            echo "Hearth Enrollment TUI (test stub)"
            echo "Would contact server at: $HEARTH_SERVER_URL"
            sleep 2
          '';
        })
      ];

      services.hearth.enrollment = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        package = pkgs.hearth-enrollment;
      };
    };
  };

  testScript = ''
    # Start the control plane
    controlplane.start()
    controlplane.wait_for_unit("hearth-api.service")
    controlplane.wait_for_open_port(3000)

    # Start the enrollment device
    device.start()
    device.wait_for_unit("multi-user.target")

    # Verify enrollment configuration was generated
    device.succeed("test -f /etc/hearth/enrollment.toml")
    device.succeed("grep 'controlplane' /etc/hearth/enrollment.toml")

    # Verify enrollment package is available
    device.succeed("which hearth-enrollment")

    # Verify enrollment user exists
    device.succeed("id enrollment")

    # Verify the device can reach the control plane health endpoint
    device.succeed("curl -sf http://controlplane:3000/health")

    # --- Enrollment API mock assertions ---

    # POST to enrollment endpoint and verify response contains enrollment_id
    device.succeed(
        "curl -sf http://controlplane:3000/api/v1/enroll "
        "-X POST -H 'Content-Type: application/json' "
        "-d '{\"hardware_id\": \"test-hw-001\"}' "
        "| grep 'enrollment_id'"
    )

    # Verify enrollment code is returned
    device.succeed(
        "curl -sf http://controlplane:3000/api/v1/enroll "
        "-X POST -H 'Content-Type: application/json' "
        "-d '{\"hardware_id\": \"test-hw-001\"}' "
        "| grep 'ABC-123'"
    )

    # Poll enrollment status endpoint
    device.succeed(
        "curl -sf http://controlplane:3000/api/v1/enroll/test-enroll-001/status "
        "| grep 'pending_approval'"
    )

    # --- Hardware detection tools ---
    device.succeed("which dmidecode")
    device.succeed("which lshw")
    device.succeed("which lspci")
    device.succeed("which lsusb")

    # --- Disk utilities ---
    device.succeed("which parted")
    device.succeed("which cryptsetup")

    # --- Enrollment TUI binary ---
    device.succeed("which hearth-enrollment")
    # Run the stub and verify it exits cleanly
    device.succeed("hearth-enrollment")

    # TODO (Phase 3+): Full enrollment flow with real binaries
    # - Verify hardware detection output
    # - Verify disk partitioning on virtual disk
    # - Verify NixOS installation from mock cache
    # - Verify reboot into installed system
  '';
}
