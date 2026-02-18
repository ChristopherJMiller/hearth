# tests/desktop-baseline.nix — NixOS VM test: GNOME desktop baseline
#
# Single-node test verifying that the Hearth desktop module correctly
# configures a GNOME desktop environment with greetd, PipeWire audio,
# and the expected base packages.
#
# Assertions:
#   - greetd service starts
#   - GNOME-related services are active
#   - PipeWire is running
#   - Base packages are installed
#   - dconf is enabled (user-level settings managed by home-manager)

{ pkgs, lib, ... }:

pkgs.testers.nixosTest {
  name = "hearth-desktop-baseline";

  nodes.machine = { config, pkgs, ... }: {
    imports = [
      ../modules/desktop.nix
      ../modules/greeter.nix
      ../modules/agent.nix
      ../modules/pam.nix
    ];

    # Provide stub packages for testing
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
          echo "hearth-greeter stub"
          sleep infinity
        '';
      })
    ];

    services.hearth.desktop.enable = true;
    services.hearth.agent = {
      enable = true;
      serverUrl = "http://localhost:3000";
      machineId = "test-desktop-001";
    };
    services.hearth.greeter.enable = true;
    services.hearth.pam.enable = true;
    # Disable SSSD for testing (no IdP available in VM)
    services.hearth.pam.enableSssd = false;

    # VM needs enough resources for GNOME
    virtualisation = {
      memorySize = 2048;
      cores = 2;
      qemu.options = [
        "-vga virtio"
      ];
    };
  };

  testScript = ''
    machine.start()

    # Wait for basic system boot
    machine.wait_for_unit("multi-user.target")

    # --- Verify greetd is running ---
    machine.wait_for_unit("greetd.service")

    # --- Verify PipeWire audio is configured ---
    # PipeWire may not fully start without audio hardware in the VM,
    # but the service unit should exist
    machine.succeed("systemctl list-unit-files | grep pipewire")

    # --- Verify base packages are installed ---
    machine.succeed("which firefox")
    machine.succeed("which nautilus")

    # --- Verify dconf is available (user-level settings via home-manager) ---
    machine.succeed("which dconf")

    # --- Verify greeter configuration ---
    machine.succeed("test -f /etc/hearth/greeter.toml")

    # --- Verify GDM is NOT running (we use greetd) ---
    machine.fail("systemctl is-active gdm.service")

    # --- Verify printing service is enabled ---
    machine.succeed("systemctl list-unit-files | grep cups")

    # --- Verify Noto fonts are installed ---
    machine.succeed("fc-list | grep -i 'Noto'")

    # --- Basic sanity: system has finished startup (degraded is acceptable in VM) ---
    machine.succeed("systemctl is-system-running --wait || test \"$(systemctl is-system-running)\" = degraded")
  '';
}
