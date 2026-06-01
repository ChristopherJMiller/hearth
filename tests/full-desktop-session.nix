# tests/full-desktop-session.nix — NixOS VM test: real cage + GNOME boot
#
# Sibling of tests/full-login-flow.nix. That test stops once the agent
# activates the closure; this one continues all the way through to a
# graphical GNOME session so we catch regressions in the cage compositor,
# greetd handoff, Wayland init, and gnome-shell startup path that the
# login-flow test deliberately skips.
#
# Costs:
#   - ~5 min cold-boot to desktop (vs ~3 min for full-login-flow)
#   - 4096 MB memory (vs 2048 MB) — GNOME needs the headroom
#
# What this test does NOT cover:
#   - UI interaction (click testing, app launching). Boot-to-desktop is
#     the goal; click-testing belongs in a separate UI-automation rig.
#   - Typing the password through cage's GTK input layer. The headless
#     test-mode in hearth-greeter still reads /tmp/hearth-test-pass —
#     once auth succeeds the greeter calls greetd.start_session() and
#     the rest of the chain (greetd → user gnome-session → gnome-shell)
#     is real production code.

{ pkgs, lib, hearth-agent, hearth-greeter, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  kanidmTest = import ./lib/kanidm-test.nix { inherit pkgs; };
  machineUuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
  machineToken = "test-machine-token-desktop";

  # Same throwaway closure shape as full-login-flow.nix — the agent's
  # `<closure>/activate` step runs as the user, and the test asserts
  # the marker files end up in the user's home.
  testUserClosure = pkgs.runCommand "hearth-test-desktop-closure" { } ''
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
  name = "hearth-full-desktop-session";

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
        ../modules/desktop.nix
      ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-agent = hearth-agent;
          hearth-greeter = hearth-greeter;
        })
      ];

      services.hearth.agent = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        machineId = machineUuid;
        pollInterval = 5;
        homeFlakeRef = "path:/etc/hearth/test-flake";
      };

      # Same path override as full-login-flow.nix — mock home-manager
      # so the agent activation succeeds without a real flake build.
      systemd.services.hearth-agent.path = lib.mkForce [
        pkgs.nix
        pkgs.util-linux
        pkgs.glibc.bin
        pkgs.getent
        pkgs.coreutils
        (pkgs.writeShellScriptBin "home-manager" ''
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

      services.hearth.greeter = {
        enable = true;
        # gnome-session is the standard graphical session entrypoint —
        # cage exits when the greeter exits, then greetd asks it to
        # spawn the user_session command which becomes gnome-session.
        sessionCommand = "${pkgs.gnome-session}/bin/gnome-session";
      };

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

      services.hearth.desktop = {
        enable = true;
        # Flatpak's activation script tries to add the Flathub remote on
        # boot which adds 30+ seconds and requires network. Off in tests.
        enableFlatpak = false;
      };

      # Headless QEMU but with software-rendered virtio-vga so cage has
      # a DRM device to bind to. -display none stops QEMU from trying
      # to open a host window; the GPU is fully software.
      virtualisation.qemu.options = [
        "-vga none"
        "-device" "virtio-vga"
        "-display" "none"
      ];

      # 4 GB is the floor for a working gnome-shell session; less and it
      # OOMs partway through extension load.
      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      virtualisation.diskSize = 8192;

      # Pre-write machine identity (simulating enrolled device).
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo -n "${machineUuid}" > /var/lib/hearth/machine-id
        echo -n "${machineToken}" > /var/lib/hearth/machine-token
      '';

      # Greeter wrapper: same headless test-mode the login-flow test
      # uses. The greeter authenticates via PAM/Kanidm with the password
      # from /tmp/hearth-test-pass, then calls greetd.start_session()
      # with the configured sessionCommand — that's the real path under
      # test here. The cage compositor still runs (it wraps the greeter
      # binary regardless of HEARTH_GREETER_TEST_MODE).
      services.greetd.settings = lib.mkForce {
        default_session = {
          command = toString (pkgs.writeShellScript "hearth-greeter-test-wrapper" ''
            export HEARTH_GREETER_TEST_MODE=1
            export HEARTH_TEST_USER="testuser@kanidm"
            export HEARTH_TEST_PASS_FILE="/tmp/hearth-test-pass"
            export HEARTH_GREETER_LOG_FILE="/tmp/hearth-greeter.log"
            export RUST_LOG="hearth_greeter=debug"
            export WLR_LIBINPUT_NO_DEVICES=1
            export XDG_SESSION_TYPE=wayland
            exec ${pkgs.cage}/bin/cage -s -- ${pkgs.hearth-greeter}/bin/hearth-greeter
          '');
          user = "greeter";
        };
        terminal.vt = 1;
      };

      virtualisation.additionalPaths = [ testUserClosure ];

      nix.settings.substituters = lib.mkForce [ ];
      nix.settings.trusted-substituters = lib.mkForce [ ];
    };
  };

  testScript = ''
    import json

    kanidm.start()
    controlplane.start()
    desktop.start()

    kanidm.wait_for_unit("kanidm.service")
    kanidm.wait_for_open_port(8443)
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    kanidm.wait_for_unit("kanidm-bootstrap.service", timeout=180)
    kanidm.succeed("test -f /tmp/bootstrap-done")
    testuser_password = kanidm.succeed("cat /tmp/testuser-password").strip()

    desktop.wait_for_unit("multi-user.target")
    desktop.wait_for_unit("seatd.service", timeout=60)
    desktop.wait_for_unit("greetd.service", timeout=60)
    desktop.wait_for_unit("hearth-agent.service")
    desktop.wait_until_succeeds("test -S /run/hearth/agent.sock", timeout=30)
    desktop.wait_for_unit("kanidm-unixd.service")
    desktop.wait_until_succeeds("getent passwd testuser@kanidm", timeout=180)

    # Stage the closure before the greeter logs in, same as login-flow.
    controlplane.succeed(
        'curl -fsS -X POST -H "Content-Type: application/json" '
        '-d \'{"username":"testuser@kanidm","base_role":"default",'
        '"latest_closure":"${testUserClosure}"}\' '
        "http://localhost:3000/api/v1/test/set-user-config"
    )

    # Trigger the headless greeter login by writing the password file.
    desktop.succeed(
        f"echo -n '{testuser_password}' > /tmp/hearth-test-pass"
    )

    # The greeter logs success and calls greetd.start_session(). Once
    # greetd accepts that and execs gnome-session, the real boot path is
    # in motion — the rest of the asserts cover regressions in the
    # cage → gnome-session → gnome-shell handoff.
    desktop.wait_until_succeeds(
        "grep -q 'session started' /tmp/hearth-greeter.log 2>/dev/null",
        timeout=180,
    )

    # --- The point of this test ---
    # gnome-session forks gnome-shell as its compositor. Both processes
    # appearing in pgrep is the proof that cage handed off to the user
    # session, gnome-session started, and gnome-shell reached its main
    # loop without crashing. The timeouts are generous because virtio-
    # vga without GL is slow.
    desktop.wait_until_succeeds("pgrep -u testuser gnome-session", timeout=120)
    desktop.wait_until_succeeds("pgrep -u testuser gnome-shell", timeout=180)

    # Snapshot the framebuffer for visual debugging when this test fails.
    # virtio-vga's text rendering is surprisingly OCR-able but we don't
    # rely on its content — flaky across GNOME minor versions.
    desktop.screenshot("desktop-after-login.png")

    # Quick liveness check: gnome-shell hasn't crashed in the last few
    # seconds. Catches the "gnome-shell prints a stack and quits 5s in"
    # regressions that pgrep alone misses.
    import time
    time.sleep(5)
    desktop.succeed("pgrep -u testuser gnome-shell")

    # Sanity gate that the agent path is still healthy under the desktop
    # load — full-login-flow already covers this, but cheap to repeat.
    desktop.succeed("systemctl is-active hearth-agent.service")
  '';
}
