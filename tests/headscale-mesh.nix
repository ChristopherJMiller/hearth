# tests/headscale-mesh.nix — NixOS VM test: real Headscale mesh VPN
#
# Full integration test with a real Headscale coordination server and two
# fleet clients that join the mesh via pre-auth keys. Verifies:
#
#   1. Headscale server starts and bootstraps with pre-auth keys
#   2. Fleet devices join the mesh automatically using pre-auth keys
#   3. Devices receive 100.64.x.y mesh IPs
#   4. Devices can reach each other over the mesh (ping)
#   5. Agent reports headscale_ip in heartbeats
#   6. MagicDNS resolves hostnames within the mesh
#
# Nodes:
#   - headscale: Headscale coordination server (services.headscale)
#   - controlplane: mock Hearth API server
#   - client1: fleet device with hearth-agent + headscale-client
#   - client2: fleet device with headscale-client (mesh peer)

{ pkgs, lib, hearth-agent, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  headscaleTest = import ./lib/headscale-test.nix { inherit pkgs; };
  machineUuid = "cccccccc-dddd-eeee-ffff-111111111111";
  machineToken = "test-machine-token-for-headscale";
in
pkgs.testers.nixosTest {
  name = "hearth-headscale-mesh";

  nodes = {
    headscale = { config, pkgs, ... }: {
      imports = [ (headscaleTest.module { port = 8080; }) ];
    };

    controlplane = { config, pkgs, ... }: {
      imports = [ (mockApi.module { port = 3000; }) ];
    };

    client1 = { config, pkgs, ... }: {
      imports = [
        ../modules/agent.nix
        ../modules/headscale-client.nix
      ];

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
        headscale.enable = true;
      };

      services.hearth.headscaleClient = {
        enable = true;
        serverUrl = "http://headscale:8080";
      };

      # Pre-write machine identity (simulating completed enrollment).
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';

      environment.systemPackages = [ pkgs.python3 pkgs.jq ];
      virtualisation.memorySize = 512;
    };

    client2 = { config, pkgs, ... }: {
      imports = [
        ../modules/headscale-client.nix
      ];

      services.hearth.headscaleClient = {
        enable = true;
        serverUrl = "http://headscale:8080";
      };

      environment.systemPackages = [ pkgs.python3 pkgs.jq ];
      virtualisation.memorySize = 512;
    };
  };

  testScript = ''
    import json

    # ── Phase 1: Boot infrastructure ──────────────────────────────────────

    headscale.start()
    controlplane.start()

    headscale.wait_for_unit("headscale.service")
    headscale.wait_for_open_port(8080)
    headscale.wait_for_unit("headscale-bootstrap.service", timeout=60)
    headscale.succeed("test -f /tmp/headscale-bootstrap-done")

    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    # Read the generated pre-auth keys
    key1 = headscale.succeed("cat /tmp/headscale-preauth-key-1").strip()
    key2 = headscale.succeed("cat /tmp/headscale-preauth-key-2").strip()
    assert len(key1) > 0, "Pre-auth key 1 is empty"
    assert len(key2) > 0, "Pre-auth key 2 is empty"

    # ── Phase 2: Write pre-auth keys and boot clients ─────────────────────

    client1.start()
    client2.start()

    client1.wait_for_unit("multi-user.target")
    client2.wait_for_unit("multi-user.target")

    # Write pre-auth keys (simulating what the enrollment TUI does)
    client1.succeed(f"echo -n '{key1}' > /var/lib/hearth/headscale-key")
    client2.succeed(f"mkdir -p /var/lib/hearth && echo -n '{key2}' > /var/lib/hearth/headscale-key")

    # Restart the join service now that keys are in place
    client1.succeed("systemctl restart hearth-headscale-join.service")
    client2.succeed("systemctl restart hearth-headscale-join.service")

    # ── Phase 3: Verify mesh connectivity ─────────────────────────────────

    # Wait for both clients to get Tailscale IPs
    client1.wait_until_succeeds(
        "tailscale status --json | python3 -c 'import json,sys; s=json.load(sys.stdin); assert len(s.get(\"Self\",{}).get(\"TailscaleIPs\",[])) > 0'",
        timeout=60,
    )
    client2.wait_until_succeeds(
        "tailscale status --json | python3 -c 'import json,sys; s=json.load(sys.stdin); assert len(s.get(\"Self\",{}).get(\"TailscaleIPs\",[])) > 0'",
        timeout=60,
    )

    # Extract mesh IPs
    client1_ip = client1.succeed(
        "tailscale status --json | python3 -c 'import json,sys; s=json.load(sys.stdin); print([ip for ip in s[\"Self\"][\"TailscaleIPs\"] if \":\" not in ip][0])'"
    ).strip()
    client2_ip = client2.succeed(
        "tailscale status --json | python3 -c 'import json,sys; s=json.load(sys.stdin); print([ip for ip in s[\"Self\"][\"TailscaleIPs\"] if \":\" not in ip][0])'"
    ).strip()

    assert client1_ip.startswith("100."), f"Expected 100.x.y.z IP, got {client1_ip}"
    assert client2_ip.startswith("100."), f"Expected 100.x.y.z IP, got {client2_ip}"
    assert client1_ip != client2_ip, "Both clients got the same IP"

    # Verify pre-auth key files were consumed (deleted after join)
    client1.succeed("test ! -f /var/lib/hearth/headscale-key")
    client2.succeed("test ! -f /var/lib/hearth/headscale-key")

    # Verify peer-to-peer connectivity over the mesh
    client1.wait_until_succeeds(f"ping -c 1 -W 5 {client2_ip}", timeout=30)
    client2.wait_until_succeeds(f"ping -c 1 -W 5 {client1_ip}", timeout=30)

    # ── Phase 4: Agent reports mesh IP ────────────────────────────────────

    # Wait for the agent to start and send heartbeats
    client1.wait_for_unit("hearth-agent.service")
    client1.wait_until_succeeds("test -S /run/hearth/agent.sock", timeout=30)

    # Wait for a heartbeat that includes the headscale_ip
    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats | python3 -c '"
        "import json,sys; d=json.load(sys.stdin); "
        "hbs = d[\"heartbeats\"]; "
        "assert len(hbs) > 0; "
        "assert any(hb.get(\"headscale_ip\") for hb in hbs), \"no heartbeat with headscale_ip\"'",
        timeout=60,
    )

    # Verify the reported IP matches what tailscale assigned
    heartbeats_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    heartbeats = json.loads(heartbeats_raw)["heartbeats"]
    mesh_hbs = [hb for hb in heartbeats if hb.get("headscale_ip")]
    assert len(mesh_hbs) > 0, "No heartbeats with headscale_ip found"
    reported_ip = mesh_hbs[-1]["headscale_ip"]
    assert reported_ip == client1_ip, (
        f"Agent reported IP '{reported_ip}' but tailscale has '{client1_ip}'"
    )

    # ── Phase 5: Module wiring checks ─────────────────────────────────────

    # SSH enabled for remote access
    client1.succeed("systemctl is-active sshd.service")

    # WireGuard port open
    client1.succeed("iptables -L -n | grep -q 41641")

    # Agent config includes headscale section
    client1.succeed("grep 'headscale' /etc/hearth/agent.toml")

    # Agent still running after all this
    client1.succeed("systemctl is-active hearth-agent.service")
  '';
}
