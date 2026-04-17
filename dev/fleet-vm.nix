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
  # Read machine ID and token from env vars (set by `just fleet-vm`).
  machineId = builtins.getEnv "HEARTH_FLEET_VM_MACHINE_ID";
  effectiveId = if machineId != "" then machineId else "00000000-0000-0000-0000-000000000001";
  machineToken = builtins.getEnv "HEARTH_FLEET_VM_MACHINE_TOKEN";
  cacheKey = builtins.getEnv "HEARTH_CACHE_PUBLIC_KEY";
in
self.lib.mkFleetHost {
  hostname = "hearth-fleet-vm";
  role = "developer";
  machineId = effectiveId;
  serverUrl = "http://api.hearth.local:3000";
  kanidmUrl = "https://kanidm.hearth.local:8443";
  kanidmCaCert = ../dev/kanidm/cert.pem;
  binaryCacheUrl = "http://cache.hearth.local:8080/hearth";
  cachePublicKey = if cacheKey != "" then cacheKey else null;
  homeFlakeRef = "path:${self}";
  roleMapping = [
    { group = "hearth-admins"; role = "admin"; }
    { group = "hearth-developers"; role = "developer"; }
    { group = "hearth-designers"; role = "designer"; }
  ];
  defaultRole = "default";
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
        diskSize = 32768; # 32GB — room for NixOS system, home-manager profiles, flatpaks, and dev tools
        resolution = { x = 1920; y = 1080; };
        qemu.options = [
          "-device" "virtio-vga-gl,xres=1920,yres=1080"
          "-display" "gtk,gl=on"
          "-device" "virtio-tablet-pci"    # absolute pointing — no mouse grab
          "-audiodev" "pipewire,id=audio0" # audio passthrough to host
          "-device" "intel-hda"
          "-device" "hda-duplex,audiodev=audio0"
        ];

        # Share a host directory for extracting logs from the VM.
        # Logs appear at dev/fleet-vm-logs/ on the host.
        sharedDirectories.logs = {
          source = "\"$FLEET_VM_LOGS\"";
          target = "/var/log/hearth-vm";
        };
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
      '' + lib.optionalString (machineToken != "") ''
        echo "${machineToken}" > /var/lib/hearth/machine-token
      '';

      # --- Fix inverted/offset mouse cursor under QEMU virtio-vga-gl ---
      # Set via multiple paths to ensure it reaches Mutter regardless of
      # how the session is launched (greetd, login shell, systemd user).
      environment.sessionVariables.MUTTER_DEBUG_FORCE_SOFTWARE_CURSOR = "1";
      environment.variables.MUTTER_DEBUG_FORCE_SOFTWARE_CURSOR = "1";
      # Also set for the greeter's cage session and the user's GNOME session
      systemd.services.greetd.environment.MUTTER_DEBUG_FORCE_SOFTWARE_CURSOR = "1";
      environment.etc."profile.d/qemu-cursor-fix.sh".text = ''
        export MUTTER_DEBUG_FORCE_SOFTWARE_CURSOR=1
      '';

      # Use virtio-tablet for absolute positioning (already in qemu.options)
      # and disable hardware cursors in the kernel framebuffer.
      boot.kernelParams = [ "vt.global_cursor_default=0" ];

      # --- QEMU guest services ---
      services.qemuGuest.enable = true;
      services.spice-vdagentd.enable = true;

      # --- Dev user (fallback for when Kanidm is unavailable) ---
      users.users.dev = {
        isNormalUser = true;
        password = "dev";
        extraGroups = [ "wheel" "hearth" "networkmanager" ];
      };
      security.sudo.wheelNeedsPassword = lib.mkForce false;

      # --- Log export: dump Hearth service logs to the shared directory ---
      systemd.services.hearth-log-export = {
        description = "Export Hearth logs to shared directory for host debugging";
        after = [ "var-log-hearth\\x2dvm.mount" ];
        requires = [ "var-log-hearth\\x2dvm.mount" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        # On start, begin tailing all hearth-related journals to files.
        # ExecStartPost spawns background journalctl processes.
        script = ''
          mkdir -p /var/log/hearth-vm

          # Export full journal continuously
          ${pkgs.systemd}/bin/journalctl -f -o short-iso \
            > /var/log/hearth-vm/journal.log 2>&1 &

          # Export hearth-specific units
          for unit in hearth-agent hearth-greeter greetd; do
            ${pkgs.systemd}/bin/journalctl -f -u "$unit" -o short-iso \
              > "/var/log/hearth-vm/$unit.log" 2>&1 &
          done
        '';
      };

      # --- Development utilities ---
      environment.systemPackages = with pkgs; [
        curl jq htop vim tmux
        spice-vdagent
      ];

      system.stateVersion = lib.mkForce "25.05";
    })
  ];
}
