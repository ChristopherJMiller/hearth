# tests/agent-heartbeat.nix — NixOS VM test: real agent heartbeat cycle
#
# Two-node test verifying that the real hearth-agent binary starts, connects
# to the mock API, sends heartbeats, and survives multiple poll cycles.
#
# Unlike agent-polling.nix (which uses stub binaries to test module wiring),
# this test runs the actual compiled hearth-agent and exercises the real
# polling loop, IPC server, and heartbeat protocol.
#
# Nodes:
#   - controlplane: runs the stateful mock API server
#   - client: runs the real hearth-agent binary with pre-written identity files
#
# Verifications:
#   - Agent service starts and stays active
#   - IPC socket is created at /run/hearth/agent.sock
#   - Mock API receives heartbeats with correct machine_id
#   - Agent survives multiple poll cycles without crashing
#   - Agent config file is generated correctly

{ pkgs, lib, hearth-agent, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  machineUuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
  machineToken = "test-machine-token-for-heartbeat";
in
pkgs.testers.nixosTest {
  name = "hearth-agent-heartbeat";

  nodes = {
    controlplane = { config, pkgs, ... }: {
      imports = [ (mockApi.module { port = 3000; }) ];
    };

    client = { config, pkgs, ... }: {
      imports = [ ../modules/agent.nix ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-agent = hearth-agent;
        })
      ];

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        machineId = machineUuid;
        pollInterval = 5;
      };

      # Pre-write machine identity files (simulating a device that
      # has already completed enrollment and provisioning).
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';
    };
  };

  testScript = ''
    import json

    controlplane.start()
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    # Verify mock API health endpoint
    controlplane.succeed("curl -sf http://localhost:3000/api/v1/health")

    client.start()
    client.wait_for_unit("multi-user.target")

    # Verify the agent configuration was generated
    client.succeed("test -f /etc/hearth/agent.toml")
    client.succeed("grep '${machineUuid}' /etc/hearth/agent.toml")

    # Verify identity files were written
    client.succeed("test -f /var/lib/hearth/machine-id")
    client.succeed("test -f /var/lib/hearth/machine-token")

    # Wait for the agent to start
    client.wait_for_unit("hearth-agent.service")

    # Verify the IPC socket is created
    client.wait_until_succeeds("test -S /run/hearth/agent.sock", timeout=30)

    # Wait for at least one heartbeat to arrive at the mock API
    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"heartbeats\"]) > 0'",
        timeout=30,
    )

    # Verify the heartbeat contains the correct machine_id
    heartbeats_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    heartbeats = json.loads(heartbeats_raw)["heartbeats"]
    assert len(heartbeats) > 0, f"Expected at least 1 heartbeat, got {len(heartbeats)}"
    assert heartbeats[0]["machine_id"] == "${machineUuid}", (
        f"Expected machine_id '${machineUuid}', got '{heartbeats[0]['machine_id']}'"
    )

    # Wait for a second heartbeat to verify the poll loop continues
    first_count = len(heartbeats)
    controlplane.wait_until_succeeds(
        f"curl -sf http://localhost:3000/api/v1/test/heartbeats | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"heartbeats\"]) > {first_count}'",
        timeout=30,
    )

    # Verify agent is still running after multiple cycles
    client.succeed("systemctl is-active hearth-agent.service")

    # Verify the hearth user was created
    client.succeed("id hearth")
  '';
}
