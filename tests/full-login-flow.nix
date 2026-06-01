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
#   7. Agent resolves user env (queries control plane, falls back to role template)
#   8. Agent activates environment via home-manager (mock in test)
#   9. Greeter starts desktop session via greetd
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

  # Per-user closure stand-in. The hearth-agent fetches the closure path
  # from /api/v1/users/<u>/env-closure, then runs `<closure>/activate` as
  # the user (see crates/hearth-agent/src/ipc.rs around the "activating
  # pre-built per-user closure" log line). For the test we hand the agent
  # a closure that just touches a couple of marker files so the test can
  # verify activation ran. The path is bind-mounted from the host's nix
  # store into the desktop VM, so `nix-store --realise` is a no-op.
  testUserClosure = pkgs.runCommand "hearth-test-user-closure" { } ''
    mkdir -p $out
    cat > $out/activate <<'SCRIPT'
    #!${pkgs.runtimeShell}
    set -eu
    mkdir -p "$HOME/.config"
    printf 'default\n' > "$HOME/.hearth-role"
    printf 'activated\n' > "$HOME/.config/hearth-activated"
    SCRIPT
    chmod +x $out/activate
  '';
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

      # kanidm version pinned via Hearth overlay (pkgs.kanidm = kanidm_1_9)

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        machineId = machineUuid;
        pollInterval = 5;
        homeFlakeRef = "path:/etc/hearth/test-flake";
      };

      # Override the agent service path to use our mock home-manager
      # instead of pkgs.home-manager (which may not exist without the
      # home-manager overlay applied to nixpkgs). Mirror the production
      # PATH from modules/agent.nix (nix + util-linux + glibc.bin +
      # getent + coreutils) so production-realistic code paths work:
      # coreutils is needed for preStart's `touch`+`chmod`, getent for
      # the username resolution in src/ipc.rs.
      systemd.services.hearth-agent.path = lib.mkForce [
        pkgs.nix
        pkgs.util-linux
        pkgs.glibc.bin
        pkgs.getent
        pkgs.coreutils
        (pkgs.writeShellScriptBin "home-manager" ''
          echo "home-manager called with args: $@" > /tmp/home-manager-invocation
          for arg in "$@"; do
            case "$arg" in
              *#*)
                role="''${arg##*#}"
                mkdir -p "$HOME/.config"
                echo "$role" > "$HOME/.hearth-role"
                echo "activated" > "$HOME/.config/hearth-activated"
                ;;
            esac
          done
          exit 0
        '')
      ];

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

      # Override greetd session command with a wrapper that sets test env vars.
      # greetd creates a clean env for the greeter, so we must inject them.
      services.greetd.settings = lib.mkForce {
        default_session = {
          command = toString (pkgs.writeShellScript "hearth-greeter-test-wrapper" ''
            export HEARTH_GREETER_TEST_MODE=1
            export HEARTH_TEST_USER="testuser@kanidm"
            export HEARTH_TEST_PASS_FILE="/tmp/hearth-test-pass"
            export HEARTH_GREETER_LOG_FILE="/tmp/hearth-greeter.log"
            export RUST_LOG="hearth_greeter=debug"
            exec ${pkgs.hearth-greeter}/bin/hearth-greeter
          '');
          user = "greeter";
        };
        terminal.vt = 1;
      };

      virtualisation.memorySize = 2048;

      # The agent realises the per-user closure at activation time. The
      # VM has no internet, and NixOS tests only stage the VM-config's
      # closure into /nix/store by default. additionalPaths makes
      # testUserClosure available in the VM's store so `nix-store
      # --realise` is a no-op against the local store.
      virtualisation.additionalPaths = [ testUserClosure ];

      # Substituters would just slow the realise call down with failed
      # cache.nixos.org lookups — the closure is already local.
      nix.settings.substituters = lib.mkForce [ ];
      nix.settings.trusted-substituters = lib.mkForce [ ];
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

    # --- Cold-boot regression gates ---
    # seatd had a 90s start-stall on cold boot (s6-notify protocol mismatch
    # confused systemd) before TimeoutStartSec=30 + Restart=on-failure was
    # added in modules/greeter.nix. Assert it reaches active within 60s so a
    # regression in those service-config knobs trips this test.
    desktop.wait_for_unit("seatd.service", timeout=60)

    # greetd depends on seatd via After=; if seatd's start fails outright,
    # greetd will stay activating. Catch that too.
    desktop.wait_for_unit("greetd.service", timeout=60)

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

    # --- Register the test user closure with the mock API ---
    # The agent fetches the closure path from GET /env-closure and runs
    # <closure>/activate as the user. Without this POST the agent's
    # wait-for-build loop runs for 300s then errors out. Doing this BEFORE
    # the password injection ensures the closure is available the moment
    # the agent first calls /env-closure.
    controlplane.succeed(
        'curl -fsS -X POST -H "Content-Type: application/json" '
        '-d \'{"username":"testuser@kanidm","base_role":"default",'
        '"latest_closure":"${testUserClosure}"}\' '
        "http://localhost:3000/api/v1/test/set-user-config"
    )

    # --- Inject password for headless greeter ---
    # The greeter polls /tmp/hearth-test-pass (greetd doesn't pass parent
    # env vars, so we use a file). Once written, the already-running greeter
    # picks it up and proceeds with the login flow.
    desktop.succeed(
        f"echo -n '{testuser_password}' > /tmp/hearth-test-pass"
    )

    # Verify the greeter logged success. The greeter writes to
    # /tmp/hearth-greeter.log via HEARTH_GREETER_LOG_FILE.
    desktop.wait_until_succeeds(
        "grep -q 'headless login succeeded\\|session started' /tmp/hearth-greeter.log 2>/dev/null",
        timeout=180,
    )

    # Verify the agent is still running
    desktop.succeed("systemctl is-active hearth-agent.service")

    # --- Verify the agent activated the per-user closure ---
    # With a closure registered up front the agent takes the "pre-built"
    # branch in crates/hearth-agent/src/ipc.rs — nix-store --realise is a
    # no-op because the closure is already in the host's bind-mounted
    # /nix/store, then it runs <closure>/activate as the user.
    desktop.succeed(
        "journalctl -u hearth-agent -o cat | grep -q 'activating pre-built per-user closure'"
    )

    # --- Verify the closure's activate script ran in the user's home ---
    # Resolve the actual home directory from the NSS passwd entry rather than
    # hardcoding it — kanidm-unixd may use UUID-based home dirs depending on
    # the home_attr setting (e.g. /home/<uuid> with a /home/<spn> symlink).
    user_home = desktop.succeed(
        "getent passwd testuser@kanidm | cut -d: -f6"
    ).strip()

    # Regression gate: `@` in /home/ broke Nextcloud Desktop and other
    # POSIX-path-naive clients, so kanidm-unixd was switched from
    # home_attr=spn to home_attr=name. Kanidm 1.10 also requires that
    # home_alias be unset — in 1.10 the alias overrides home_attr in
    # token_homedirectory() (resolver.rs:782-785). The on-disk path
    # must be the local part of the SPN — /home/testuser, not
    # /home/testuser@kanidm.
    assert user_home == "/home/testuser", (
        f"Expected home directory /home/testuser (local part of SPN), "
        f"got {user_home!r}. Check kanidm-unixd home_attr is 'name' "
        f"and home_alias is unset (Kanidm 1.10 alias semantics changed)."
    )

    desktop.wait_until_succeeds(
        f"test -f {user_home}/.hearth-role",
        timeout=10,
    )
    role = desktop.succeed(f"cat {user_home}/.hearth-role").strip()
    assert role == "default", (
        f"Expected role 'default' in marker file, got '{role}'"
    )
  '';
}
