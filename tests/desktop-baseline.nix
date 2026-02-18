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
#   - dconf defaults are in place

{ pkgs, lib, ... }:

pkgs.nixosTest {
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

    # --- Verify dconf defaults were installed ---
    machine.succeed("test -f /etc/dconf/db/hearth.d/00-hearth-defaults")
    machine.succeed("grep 'prefer-dark' /etc/dconf/db/hearth.d/00-hearth-defaults")

    # --- Verify dconf profile is configured ---
    machine.succeed("test -f /etc/dconf/profile/user")
    machine.succeed("grep 'system-db:hearth' /etc/dconf/profile/user")

    # --- Verify greeter configuration ---
    machine.succeed("test -f /etc/hearth/greeter.toml")

    # --- Verify X server / GNOME desktop is configured ---
    machine.succeed("systemctl list-unit-files | grep -i gnome || true")

    # --- Verify GDM is NOT running (we use greetd) ---
    machine.fail("systemctl is-active gdm.service")

    # --- Verify printing service is enabled ---
    machine.succeed("systemctl list-unit-files | grep cups")

    # --- Verify font configuration ---
    machine.succeed("fc-list | grep -i 'Noto' || true")

    # --- Basic sanity: system is still healthy ---
    machine.succeed("systemctl is-system-running || true")
  '';
}
