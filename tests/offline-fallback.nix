# tests/offline-fallback.nix — NixOS VM test: offline resilience with real agent
#
# Two-node test verifying that the real hearth-agent gracefully handles
# network loss by queuing heartbeats to SQLite, and drains the queue
# when connectivity is restored.
#
# Nodes:
#   - controlplane: runs the stateful mock API server
#   - client: runs the real hearth-agent binary
#
# Test phases:
#   1. Online: agent sends heartbeats normally
#   2. Offline: iptables blocks traffic, agent queues to SQLite
#   3. Reconnect: traffic restored, queued events drain to mock API

{ pkgs, lib, hearth-agent, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  machineUuid = "11111111-2222-3333-4444-555555555555";
  machineToken = "test-machine-token-for-offline";
in
pkgs.testers.nixosTest {
  name = "hearth-offline-fallback";

  nodes = {
    controlplane = { config, pkgs, ... }: {
      imports = [ (mockApi.module { port = 3000; }) ];
      environment.systemPackages = [ pkgs.python3 pkgs.curl ];
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

      # Pre-write machine identity files (simulating enrolled device)
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';

      # sqlite3 needed for queue inspection assertions
      environment.systemPackages = [ pkgs.sqlite ];
    };
  };

  testScript = ''
    import json

    # ── Phase 1: Online operation ──────────────────────────────────────

    controlplane.start()
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    client.start()
    client.wait_for_unit("multi-user.target")
    client.wait_for_unit("hearth-agent.service")

    # Wait for at least 2 heartbeats (proves the poll loop works)
    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
        " | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"heartbeats\"]) >= 2'",
        timeout=60,
    )

    # Verify correct machine_id in heartbeats
    heartbeats_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    heartbeats = json.loads(heartbeats_raw)["heartbeats"]
    assert len(heartbeats) >= 2, f"Expected >=2 heartbeats, got {len(heartbeats)}"
    assert heartbeats[0]["machine_id"] == "${machineUuid}", (
        f"Wrong machine_id: {heartbeats[0]['machine_id']}"
    )

    # ── Phase 2: Network disruption ────────────────────────────────────

    # Reset heartbeat log so we can cleanly measure reconnection
    controlplane.succeed(
        "curl -sf -X POST http://localhost:3000/api/v1/test/reset-heartbeats"
    )

    # Block all traffic from client to controlplane
    client.succeed("iptables -A OUTPUT -d controlplane -j REJECT")

    # Wait long enough for several failed poll cycles (5s interval × 4+)
    client.sleep(25)

    # Agent must survive the outage
    client.succeed("systemctl is-active hearth-agent.service")

    # SQLite queue should have accumulated heartbeat events
    queue_count = int(client.succeed(
        "sqlite3 /var/lib/hearth/queue.db 'SELECT COUNT(*) FROM event_queue'"
    ).strip())
    assert queue_count > 0, f"Expected queued events, got {queue_count}"

    # Mock API should have received 0 heartbeats during the outage
    zero_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    zero_heartbeats = json.loads(zero_raw)["heartbeats"]
    assert len(zero_heartbeats) == 0, (
        f"Expected 0 heartbeats during outage, got {len(zero_heartbeats)}"
    )

    # ── Phase 3: Reconnection & queue drain ────────────────────────────

    # Restore network
    client.succeed("iptables -D OUTPUT -d controlplane -j REJECT")

    # Wait for heartbeats to appear (queued events drain + fresh heartbeats)
    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
        " | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"heartbeats\"]) >= 1'",
        timeout=60,
    )

    # Verify heartbeats arrived after reconnection
    reconnect_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    reconnect_heartbeats = json.loads(reconnect_raw)["heartbeats"]
    assert len(reconnect_heartbeats) >= 1, (
        f"Expected >=1 heartbeats after reconnection, got {len(reconnect_heartbeats)}"
    )

    # Queue should be drained now
    client.wait_until_succeeds(
        "test $(sqlite3 /var/lib/hearth/queue.db 'SELECT COUNT(*) FROM event_queue') -eq 0",
        timeout=30,
    )

    # Agent must still be running
    client.succeed("systemctl is-active hearth-agent.service")
  '';
}
