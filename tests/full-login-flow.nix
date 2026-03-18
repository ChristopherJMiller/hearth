# tests/full-login-flow.nix — NixOS VM test: end-to-end login flow
#
# Three-node test combining Kanidm identity management, mock API control
# plane, and a full Hearth desktop with real agent and greeter binaries.
#
# Tests the complete user login lifecycle:
#   1. Kanidm server bootstraps with test users and groups
#   2. Mock API accepts heartbeats from the agent
#   3. Desktop boots with real hearth-agent (heartbeats, IPC socket)
#   4. kanidm-unixd resolves the test user via NSS
#   5. hearth-greeter (headless mode) authenticates via greetd → PAM → Kanidm
#   6. Greeter requests environment prep from agent via IPC
#   7. Agent sends Ready (no home_flake_ref = instant)
#   8. Greeter starts desktop session via greetd
#
# Nodes:
#   - kanidm: Kanidm identity server with TLS + bootstrap
#   - controlplane: stateful mock API server
#   - desktop: full Hearth stack (agent, greeter, PAM/kanidm)

{ pkgs, lib, hearth-agent, hearth-greeter, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  kanidmTest = import ./lib/kanidm-test.nix { inherit pkgs; };
  machineUuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
  machineToken = "test-machine-token-login-flow";
in
pkgs.testers.nixosTest {
  name = "hearth-full-login-flow";

  nodes = {
    kanidm = { config, pkgs, ... }: {
      imports = [ (kanidmTest.module {}) ];
    };

    controlplane = { config, pkgs, ... }: {
      imports = [ (mockApi.module { port = 3000; }) ];
    };

    desktop = { config, pkgs, ... }: {
      imports = [
        ../modules/agent.nix
        ../modules/greeter.nix
        ../modules/pam.nix
        ../modules/kanidm-client.nix
      ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-agent = hearth-agent;
          hearth-greeter = hearth-greeter;
        })
      ];

      services.kanidm.package = pkgs.kanidm_1_7;

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        machineId = machineUuid;
        pollInterval = 5;
      };

      services.hearth.greeter.enable = true;

      services.hearth.pam = {
        enable = true;
        authBackend = "kanidm";
      };

      services.hearth.kanidmClient = {
        enable = true;
        uri = "https://kanidm:8443";
        caCertPath = kanidmTest.caCertPath;
        allowedLoginGroups = [ "hearth-users" ];
        hsmType = "soft";
      };

      # Pre-write machine identity (simulating enrolled device)
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';

      # Run greeter in headless test mode. The HEARTH_TEST_USER is static
      # but HEARTH_TEST_PASS must be injected at runtime after Kanidm
      # bootstrap completes (it may use a recovered random password).
      # The test script stops greetd, sets the env, and restarts it.
      systemd.services.greetd.environment = {
        HEARTH_GREETER_TEST_MODE = "1";
        HEARTH_TEST_USER = "testuser@kanidm";
      };

      virtualisation.memorySize = 2048;
    };
  };

  testScript = ''
    import json

    # Boot all three nodes in parallel
    kanidm.start()
    controlplane.start()
    desktop.start()

    # Wait for infrastructure to be ready
    kanidm.wait_for_unit("kanidm.service")
    kanidm.wait_for_open_port(8443)
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    # Wait for Kanidm bootstrap (creates users, groups, sets passwords)
    kanidm.wait_for_unit("kanidm-bootstrap.service", timeout=120)
    kanidm.succeed("test -f /tmp/bootstrap-done")
    testuser_password = kanidm.succeed("cat /tmp/testuser-password").strip()

    # Wait for desktop to reach multi-user (agent, kanidm-unixd start here)
    desktop.wait_for_unit("multi-user.target")

    # --- Verify agent heartbeats ---
    desktop.wait_for_unit("hearth-agent.service")
    desktop.wait_until_succeeds("test -S /run/hearth/agent.sock", timeout=30)

    controlplane.wait_until_succeeds(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
        " | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d[\"heartbeats\"]) > 0'",
        timeout=30,
    )

    heartbeats_raw = controlplane.succeed(
        "curl -sf http://localhost:3000/api/v1/test/heartbeats"
    )
    heartbeats = json.loads(heartbeats_raw)["heartbeats"]
    assert heartbeats[0]["machine_id"] == "${machineUuid}", (
        f"Expected machine_id '${machineUuid}', got '{heartbeats[0]['machine_id']}'"
    )

    # --- Verify Kanidm user resolution ---
    desktop.wait_for_unit("kanidm-unixd.service")
    desktop.wait_until_succeeds(
        "getent passwd testuser@kanidm",
        timeout=120,
    )

    # --- Inject password and restart greetd for headless login ---
    # Stop greetd, set the password env var (only visible to services
    # started after this call), then restart greetd.
    desktop.succeed("systemctl stop greetd.service")
    desktop.succeed(
        f"systemctl set-environment HEARTH_TEST_PASS='{testuser_password}'"
    )
    desktop.succeed("systemctl start greetd.service")

    # Wait for greetd to start the headless greeter, which will:
    #   1. Connect to greetd IPC (GREETD_SOCK set by greetd)
    #   2. Create session for testuser@kanidm
    #   3. Authenticate via PAM → kanidm-unixd → Kanidm server
    #   4. Resolve user groups via NSS
    #   5. Connect to agent IPC → PrepareUserEnv → Ready (instant)
    #   6. Start desktop session via greetd
    desktop.wait_for_unit("greetd.service")

    # Verify the greeter logged success (not crash-looping)
    desktop.wait_until_succeeds(
        "journalctl -u greetd -o cat | grep -q 'headless login succeeded\\|session started'",
        timeout=30,
    )

    # Verify the agent is still running
    desktop.succeed("systemctl is-active hearth-agent.service")
  '';
}
