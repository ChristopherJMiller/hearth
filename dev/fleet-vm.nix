# dev/fleet-vm.nix — Pre-enrolled fleet host VM for rapid development
#
# Uses mkFleetHost (the same builder as the build pipeline) to produce a
# VM that's fully connected to the dev control plane: agent polling,
# Kanidm auth, greeter, GNOME desktop, binary cache — everything.
#
# Skips enrollment/disko/nixos-install — the VM boots directly as a
# fleet machine with a pre-seeded machine identity.
#
# Requirements:
#   - `just demo` must be running (API, Kanidm, Attic, etc.)
#
# Usage:
#   just fleet-vm     (registers machine in DB, boots VM with fresh UUID)
#   # or manually: HEARTH_FLEET_VM_MACHINE_ID=<uuid> nix run --impure .#fleet-vm

{ self, nixpkgs, system ? "x86_64-linux" }:

let
  # Read machine ID from env var (set by `just fleet-vm`) or use a default.
  machineId = builtins.getEnv "HEARTH_FLEET_VM_MACHINE_ID";
  effectiveId = if machineId != "" then machineId else "00000000-0000-0000-0000-000000000001";
in
self.lib.mkFleetHost {
  hostname = "hearth-fleet-vm";
  role = "developer";
  machineId = effectiveId;
  serverUrl = "http://api.hearth.local:3000";
  kanidmUrl = "https://kanidm.hearth.local:8443";
  kanidmCaCert = ../dev/kanidm/cert.pem;
  binaryCacheUrl = "http://cache.hearth.local:8080/hearth";
  enableDesktop = true;
  matrixUrl = "http://chat.hearth.local";
  matrixServerName = "hearth.local";
  nextcloudUrl = "http://cloud.hearth.local";

  extraModules = [
    # Import the QEMU virtualisation module for `nix run .#fleet-vm`
    ({ modulesPath, ... }: { imports = [ (modulesPath + "/virtualisation/qemu-vm.nix") ]; })

    # VM-specific overrides
    ({ config, pkgs, lib, ... }: {
      # --- QEMU VM settings ---
      virtualisation = {
        memorySize = 4096;
        cores = 2;
        graphics = true;
        qemu.options = [
          "-device" "virtio-vga-gl"
          "-display" "gtk,gl=on"
        ];
      };

      # --- Network: resolve *.hearth.local to QEMU host gateway ---
      networking.hosts."10.0.2.2" = [
        "api.hearth.local"
        "cache.hearth.local"
        "kanidm.hearth.local"
        "chat.hearth.local"
        "cloud.hearth.local"
        "grafana.hearth.local"
      ];
      networking.firewall.enable = lib.mkForce false;

      # --- Trust the Hearth Dev CA (Caddy's internal root) ---
      security.pki.certificateFiles =
        lib.optional (builtins.pathExists ../dev/caddy/root.crt) ../dev/caddy/root.crt;

      # --- Pre-seed machine identity ---
      # The agent reads these on startup instead of going through enrollment.
      system.activationScripts.hearth-identity = ''
        mkdir -p /var/lib/hearth
        echo "${effectiveId}" > /var/lib/hearth/machine-id
      '';

      # --- Dev user (fallback for when Kanidm is unavailable) ---
      users.users.dev = {
        isNormalUser = true;
        password = "dev";
        extraGroups = [ "wheel" "hearth" "networkmanager" ];
      };
      security.sudo.wheelNeedsPassword = lib.mkForce false;

      # --- Development utilities ---
      environment.systemPackages = with pkgs; [
        curl jq htop vim tmux
      ];

      system.stateVersion = lib.mkForce "25.05";
    })
  ];
}
