# dev/fleet-vm.nix — Lightweight dev VM running hearth-agent
#
# A microvm.nix-based VM for interactive development of the hearth-agent.
# Runs a minimal system (no desktop) with the agent configured to talk
# to a hearth-api server running on the host at localhost:3000.
#
# Usage (from the hearth repo root):
#   nix run .#fleet-vm
#
# Or build and run manually:
#   nix build .#fleet-vm
#   ./result/bin/microvm-run
#
# The VM uses virtiofs to share the Nix store with the host for fast
# iteration, and forwards port 3000 from guest to host for API access.

{ config, lib, pkgs, ... }:

{
  # --- MicroVM configuration ---
  microvm = {
    # Hypervisor: qemu is the most portable option
    hypervisor = "qemu";

    # Resource allocation for a minimal agent VM
    mem = 512; # MB — agent is lightweight
    vcpu = 1;

    # Network: user-mode networking with port forwarding
    # The guest can reach the host's API server via the gateway
    interfaces = [
      {
        type = "user";
        id = "hearth-dev";
        mac = "02:00:00:00:00:01";
      }
    ];

    # Share the host's Nix store for fast iteration
    shares = [
      {
        tag = "ro-store";
        source = "/nix/store";
        mountPoint = "/nix/.ro-store";
        proto = "virtiofs";
      }
    ];

    # Writable overlay for the VM's Nix store
    volumes = [
      {
        image = "hearth-dev-state.img";
        mountPoint = "/var/lib/hearth";
        size = 256; # MB
      }
    ];
  };

  # --- System configuration ---
  networking = {
    hostName = "hearth-dev-vm";
    useDHCP = true;
    firewall.enable = false; # Dev VM, no need for firewall
  };

  # --- Hearth agent ---
  imports = [ ../modules/agent.nix ];

  nixpkgs.overlays = [
    (final: prev: {
      # Use the locally-built agent package, or fall back to a stub
      hearth-agent = pkgs.hearth-agent or (prev.writeShellScriptBin "hearth-agent" ''
        echo "hearth-agent dev stub starting..."
        SOCKET_PATH="''${HEARTH_SOCKET_PATH:-/run/hearth/agent.sock}"
        mkdir -p "$(dirname "$SOCKET_PATH")"

        # Signal systemd readiness
        if [ -n "$NOTIFY_SOCKET" ]; then
          ${prev.systemd}/bin/systemd-notify --ready
        fi

        echo "Polling ''${HEARTH_CONFIG:=/etc/hearth/agent.toml}"
        while true; do
          echo "[$(date -Iseconds)] heartbeat to ''${HEARTH_SERVER_URL:-http://10.0.2.2:3000}"
          ${prev.curl}/bin/curl -sf --connect-timeout 3 \
            "''${HEARTH_SERVER_URL:-http://10.0.2.2:3000}/api/v1/health" 2>/dev/null \
            && echo "  -> server reachable" \
            || echo "  -> server unreachable"
          sleep 10
        done
      '');
    })
  ];

  services.hearth.agent = {
    enable = true;
    # 10.0.2.2 is the default gateway in QEMU user-mode networking,
    # which routes to the host — where the dev API server runs
    serverUrl = "http://10.0.2.2:3000";
    machineId = "dev-fleet-vm-001";
    pollInterval = 10;
    logLevel = "debug";
  };

  # --- Development utilities ---
  environment.systemPackages = with pkgs; [
    curl
    jq
    htop
    vim
    tmux
    tcpdump
    strace
  ];

  # --- Minimal system settings ---
  users.users.dev = {
    isNormalUser = true;
    password = "dev";
    extraGroups = [ "wheel" "hearth" ];
  };

  security.sudo.wheelNeedsPassword = false;

  # Auto-login for quick development iteration
  services.getty.autologinUser = "dev";

  # Show useful info on login
  environment.etc."motd" = {
    text = ''

      === Hearth Fleet Dev VM ===

      Agent status:   systemctl status hearth-agent
      Agent logs:     journalctl -fu hearth-agent
      Agent config:   cat /etc/hearth/agent.toml
      Agent socket:   /run/hearth/agent.sock

      Host API:       http://10.0.2.2:3000

    '';
  };

  # Enable Nix flakes for development
  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  system.stateVersion = "25.05";
}
