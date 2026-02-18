# dev/enrollment-vm.nix — Lightweight dev VM for testing enrollment
#
# A microvm.nix-based VM that runs the hearth-enrollment TUI for
# interactive development and testing of the enrollment flow.
# Configured with DHCP networking and auto-starts the enrollment TUI.
#
# Usage (from the hearth repo root):
#   nix run .#enrollment-vm
#
# The VM simulates a bare-metal device going through initial enrollment.
# It contacts the host's API server at localhost:3000 for enrollment.

{ config, lib, pkgs, ... }:

{
  # --- MicroVM configuration ---
  microvm = {
    hypervisor = "qemu";

    # Enrollment needs a bit more memory for disk operations
    mem = 1024; # MB
    vcpu = 1;

    # Network: user-mode networking
    interfaces = [
      {
        type = "user";
        id = "hearth-enroll";
        mac = "02:00:00:00:00:02";
      }
    ];

    # Share the host's Nix store
    shares = [
      {
        tag = "ro-store";
        source = "/nix/store";
        mountPoint = "/nix/.ro-store";
        proto = "virtiofs";
      }
    ];

    # A disk image to simulate the target installation disk
    volumes = [
      {
        image = "hearth-enroll-target.img";
        mountPoint = "/mnt/target";
        size = 4096; # 4GB for simulated NixOS install target
      }
      {
        image = "hearth-enroll-state.img";
        mountPoint = "/var/lib/hearth";
        size = 256;
      }
    ];
  };

  # --- System configuration ---
  networking = {
    hostName = "hearth-enroll-vm";
    useDHCP = true;
    firewall.enable = false;
  };

  # --- Enrollment module ---
  imports = [ ../modules/enrollment.nix ];

  nixpkgs.overlays = [
    (final: prev: {
      hearth-enrollment = pkgs.hearth-enrollment or (prev.writeShellScriptBin "hearth-enrollment" ''
        # Enrollment TUI dev stub
        clear
        echo "============================================"
        echo "  Hearth Device Enrollment"
        echo "============================================"
        echo ""
        echo "Server: ''${HEARTH_SERVER_URL:-http://10.0.2.2:3000}"
        echo ""

        # Simulate hardware detection
        echo "[1/5] Detecting hardware..."
        echo "  CPU: $(${prev.coreutils}/bin/nproc) cores"
        echo "  RAM: $(free -h | awk '/^Mem:/ {print $2}')"
        echo "  Disk: $(lsblk -d -n -o SIZE /dev/vda 2>/dev/null || echo 'N/A')"
        echo ""
        sleep 1

        # Simulate network check
        echo "[2/5] Checking network connectivity..."
        if ${prev.curl}/bin/curl -sf --connect-timeout 5 \
             "''${HEARTH_SERVER_URL:-http://10.0.2.2:3000}/api/v1/health" 2>/dev/null; then
          echo "  Control plane reachable"
        else
          echo "  WARNING: Control plane not reachable"
          echo "  Make sure hearth-api is running on the host"
        fi
        echo ""
        sleep 1

        # Simulate enrollment
        echo "[3/5] Requesting enrollment..."
        echo "  Enrollment code: DEV-$(${prev.coreutils}/bin/head -c 3 /dev/urandom | ${prev.coreutils}/bin/od -A n -t x1 | tr -d ' \n' | tr '[:lower:]' '[:upper:]')"
        echo ""
        echo "  Enter this code in the Hearth web console to approve."
        echo ""
        sleep 2

        echo "[4/5] Waiting for approval... (press Enter to simulate approval)"
        read -r

        echo "[5/5] Enrollment complete (simulated)."
        echo ""
        echo "In production, the system would now:"
        echo "  - Partition the target disk"
        echo "  - Download the NixOS closure"
        echo "  - Install NixOS"
        echo "  - Enroll Secure Boot and TPM"
        echo "  - Reboot into the installed system"
        echo ""
        echo "Press Enter to exit..."
        read -r
      '');
    })
  ];

  services.hearth.enrollment = {
    enable = true;
    serverUrl = "http://10.0.2.2:3000";
    package = pkgs.hearth-enrollment;
    autoStart = true;
  };

  # --- Development utilities (available if user drops to shell) ---
  environment.systemPackages = with pkgs; [
    curl
    jq
    vim
    parted
    lsblk
    dmidecode
    pciutils
    usbutils
    lshw
  ];

  # --- Console configuration ---
  console = {
    font = "Lat2-Terminus16";
    keyMap = "us";
  };

  # Show info on non-TTY1 terminals
  environment.etc."motd" = {
    text = ''

      === Hearth Enrollment Dev VM ===

      The enrollment TUI runs on TTY1.
      Switch to TTY1: Alt+F1

      Enrollment config: cat /etc/hearth/enrollment.toml
      Host API:          http://10.0.2.2:3000
      Target disk:       /mnt/target (simulated)

    '';
  };

  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  system.stateVersion = "25.05";
}
