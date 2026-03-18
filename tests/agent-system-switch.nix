# tests/agent-system-switch.nix — NixOS VM test: agent applies a system closure
#
# Verifies the most critical agent code path: receiving a target closure from
# the control plane and applying it via `nix-env --set` + `switch-to-configuration switch`.
#
# Key insight: we use the VM's own running system closure (/run/current-system)
# as the target. It's already in the store and has bin/switch-to-configuration.
# The agent starts with current_closure = None, sees the target, runs the full
# update pipeline, and reports the new closure in subsequent heartbeats.
#
# Nodes:
#   - controlplane: runs the stateful mock API server
#   - client: runs the real hearth-agent with pre-written identity files
#
# Verifications:
#   - Agent applies the system closure (nix-env + switch-to-configuration)
#   - Heartbeat reports current_closure matching the target
#   - System profile symlink is updated
#   - Agent survives the switch and continues polling
#   - No re-application on subsequent cycles (idempotency)

{ pkgs, lib, hearth-agent, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  machineUuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
  machineToken = "test-machine-token-for-system-switch";
in
pkgs.testers.nixosTest {
  name = "hearth-agent-system-switch";

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

      # NixOS test VMs disable switch-to-configuration by default (Hydra
      # optimization). We need it for the system update pipeline.
      system.switch.enable = true;

      # Disable boot loader installation — there's no real disk in the VM
      # and grub-install would fail with "will not proceed with blocklists".
      boot.loader.grub.enable = false;

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        machineId = machineUuid;
        pollInterval = 5;
      };

      # Pre-write machine identity files (simulating completed enrollment)
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';
    };
  };

  testScript = ''
    # ── Phase 0: Start infrastructure ──────────────────────────────────
    controlplane.start()
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    client.start()
    client.wait_for_unit("multi-user.target")
    client.wait_for_unit("hearth-agent.service")

    # ── Phase 1: Baseline — agent heartbeats with no current closure ───
    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
        " | python3 -c 'import json,sys; d=json.load(sys.stdin);"
        " hbs=d[\"heartbeats\"]; assert len(hbs) > 0;"
        " assert hbs[0][\"current_closure\"] is None'",
        timeout=30,
    )

    # ── Phase 2: Set target closure to the VM's own system ─────────────
    # /run/current-system is the running NixOS closure — it's already in
    # the Nix store and has bin/switch-to-configuration.
    target = client.succeed("readlink -f /run/current-system").strip()
    assert target.startswith("/nix/store/"), f"Unexpected current-system path: {target}"

    # Verify the closure has switch-to-configuration (enabled via system.switch.enable)
    client.succeed(f"test -x {target}/bin/switch-to-configuration")

    controlplane.succeed(
        f"curl -sf -X POST http://localhost:3000/api/v1/test/set-target-closure"
        f" -H 'Content-Type: application/json'"
        f" -d '{{\"target_closure\": \"{target}\"}}'"
    )

    # Clear heartbeat log so we only look at post-update heartbeats
    controlplane.succeed(
        "curl -sf -X POST http://localhost:3000/api/v1/test/reset-heartbeats"
    )

    # ── Phase 3: Wait for agent to apply and report the closure ────────
    # The agent will: fetch target-state, run nix-env --set, run
    # switch-to-configuration switch, then heartbeat with the new closure.
    # switch-to-configuration may restart the agent service, so we use a
    # generous timeout and wait_until_succeeds.
    controlplane.wait_until_succeeds(
        f"curl -sf http://localhost:3000/api/v1/test/heartbeats"
        f" | python3 -c '"
        f"import json,sys; d=json.load(sys.stdin);"
        f"hbs = d[\"heartbeats\"];"
        f"assert any(h.get(\"current_closure\") == \"{target}\" for h in hbs),"
        f" f\"No heartbeat with target closure yet ({{len(hbs)}} heartbeats)\"'",
        timeout=120,
    )

    # Verify the system profile symlink was updated
    client.wait_until_succeeds(
        f"readlink -f /nix/var/nix/profiles/system | grep -q '{target}'",
        timeout=30,
    )

    # Verify agent is still running after the switch
    client.wait_until_succeeds(
        "systemctl is-active hearth-agent.service",
        timeout=60,
    )

    # ── Phase 4: Idempotency — no re-application ──────────────────────
    # Reset heartbeats and wait for a few more cycles. The agent should
    # see current_closure == target_closure and skip the update.
    controlplane.succeed(
        "curl -sf -X POST http://localhost:3000/api/v1/test/reset-heartbeats"
    )

    # Wait for 2+ heartbeats and verify all report the correct closure
    controlplane.wait_until_succeeds(
        f"curl -sf http://localhost:3000/api/v1/test/heartbeats"
        f" | python3 -c '"
        f"import json,sys; d=json.load(sys.stdin);"
        f"hbs=d[\"heartbeats\"]; assert len(hbs) >= 2,"
        f" f\"only {{len(hbs)}} heartbeats\";"
        f"bad=[i for i,h in enumerate(hbs) if h.get(\"current_closure\") != \"{target}\"];"
        f"assert not bad, f\"heartbeats {{bad}} have wrong closure\"'",
        timeout=30,
    )
  '';
}
