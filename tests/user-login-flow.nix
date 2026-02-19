# tests/user-login-flow.nix — NixOS VM test stub: user login flow
#
# This test will verify the complete user login lifecycle through greetd
# and hearth-greeter:
#
# 1. Machine boots with hearth-greeter showing the login screen
# 2. User credentials are submitted via greetd IPC
# 3. PAM authenticates the user (against local users in test, SSSD in prod)
# 4. hearth-greeter contacts hearth-agent for environment preparation
# 5. hearth-agent creates the home directory
# 6. hearth-agent activates the role profile (or per-user closure)
# 7. hearth-agent reports Ready to the greeter
# 8. hearth-greeter sends StartSession to greetd
# 9. GNOME session launches with the prepared environment
# 10. User's home directory has correct XDG structure and dotfiles
#
# Nodes:
#   - machine: full Hearth desktop with greeter, agent, and GNOME
#
# NOTE: This is a structural stub. The greetd IPC interaction requires
# the greeter and agent binaries to be functional. The test framework
# and assertions are defined for when those components are ready.

{ pkgs, lib, ... }:

pkgs.testers.nixosTest {
  name = "hearth-user-login-flow";

  nodes.machine = { config, pkgs, ... }: {
    imports = [
      ../modules/agent.nix
      ../modules/greeter.nix
      ../modules/desktop.nix
      ../modules/pam.nix
    ];

    nixpkgs.overlays = [
      (final: prev: {
        hearth-agent = prev.writeShellScriptBin "hearth-agent" ''
          mkdir -p /run/hearth
          if [ -n "$NOTIFY_SOCKET" ]; then
            ${prev.systemd}/bin/systemd-notify --ready
          fi
          sleep infinity
        '';
        hearth-greeter = prev.writeShellScriptBin "hearth-greeter" ''
          echo "hearth-greeter stub - login flow test"
          sleep infinity
        '';
      })
    ];

    services.hearth.agent = {
      enable = true;
      serverUrl = "http://localhost:3000";
      machineId = "test-login-001";
    };
    services.hearth.greeter.enable = true;
    services.hearth.desktop.enable = true;
    services.hearth.pam.enable = true;
    services.hearth.pam.authBackend = "none";

    # Create a test user for login testing
    users.users.testuser = {
      isNormalUser = true;
      home = "/home/testuser";
      password = "testpass123";
      extraGroups = [ "users" ];
    };

    virtualisation = {
      memorySize = 2048;
      cores = 2;
    };
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # Verify greetd is running (the greeter entry point)
    machine.wait_for_unit("greetd.service")

    # Verify the agent is running
    machine.wait_for_unit("hearth-agent.service")

    # Verify agent runtime directory exists
    machine.succeed("test -d /run/hearth")

    # Verify the test user exists
    machine.succeed("id testuser")

    # Verify PAM configuration is in place
    machine.succeed("test -f /etc/hearth/agent.toml")
    machine.succeed("test -f /etc/hearth/greeter.toml")

    # TODO (Phase 1+): Full login flow assertions
    # These require functional greeter and agent binaries:
    #
    # - Verify greeter renders login screen (screenshot comparison)
    # - Submit credentials via greetd IPC test helper
    # - Wait for agent to prepare user environment
    # - Verify home directory creation at /home/testuser
    # - Verify XDG directories exist
    # - Verify role profile activation (dotfiles, symlinks)
    # - Verify GNOME session starts
    # - Verify dconf settings are applied
    # - Test logout and re-login (cached closure path)
  '';
}
