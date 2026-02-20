# modules/enrollment.nix — NixOS module for enrollment image configuration
#
# Builds a minimal NixOS system for device enrollment. This runs the
# hearth-enrollment TUI, which handles hardware detection, network setup,
# control plane registration, and disk provisioning.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.enrollment;
in
{
  options.services.hearth.enrollment = {
    enable = lib.mkEnableOption "Hearth device enrollment system";

    serverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://api.hearth.example.com";
      description = "URL of the Hearth control plane enrollment endpoint.";
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.hearth-enrollment;
      defaultText = lib.literalExpression "pkgs.hearth-enrollment";
      description = "The hearth-enrollment package to use.";
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to automatically start the enrollment TUI on boot.";
    };

    wifiSupport = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to include WiFi firmware and tools for enrollment.";
    };

    kanidmUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "https://idm.hearth.example.com";
      description = ''
        URL of the Kanidm server for OAuth2 device authorization flow.
        When set, the enrollment TUI will require user authentication
        before device enrollment.
      '';
    };

    kanidmClientId = lib.mkOption {
      type = lib.types.str;
      default = "hearth-enrollment";
      description = "OAuth2 client ID for the enrollment device flow.";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- Minimal system: no desktop, just what enrollment needs ---

    # Networking: DHCP on all interfaces
    networking = {
      useDHCP = lib.mkDefault true;
      hostName = "hearth-enrollment";
      # Enable NetworkManager for WiFi support during enrollment
      networkmanager.enable = cfg.wifiSupport;
      # Basic firewall: enrollment only makes outbound connections
      firewall.enable = true;
    };

    # WiFi firmware if needed
    hardware.enableRedistributableFirmware = lib.mkIf cfg.wifiSupport true;

    # --- Auto-login as root ---
    # The enrollment ISO is a single-purpose system. Disk partitioning,
    # formatting, and nixos-install all require root, so we auto-login
    # as root directly rather than using sudo.
    services.getty.autologinUser = lib.mkIf cfg.autoStart "root";

    # --- Auto-start enrollment TUI on TTY1 ---
    programs.bash.interactiveShellInit = lib.mkIf cfg.autoStart ''
      if [ "$(tty)" = "/dev/tty1" ] && [ -z "$HEARTH_ENROLLMENT_STARTED" ]; then
        export HEARTH_ENROLLMENT_STARTED=1
        export HEARTH_SERVER_URL="${cfg.serverUrl}"
        ${lib.optionalString (cfg.kanidmUrl != null) ''
        export HEARTH_KANIDM_URL="${cfg.kanidmUrl}"
        export HEARTH_KANIDM_CLIENT_ID="${cfg.kanidmClientId}"
        ''}
        exec ${cfg.package}/bin/hearth-enrollment
      fi
    '';

    # --- Enrollment configuration ---
    environment.etc."hearth/enrollment.toml" = {
      text = ''
        [server]
        url = "${cfg.serverUrl}"

        [enrollment]
        # Hardware detection is automatic
        auto_detect_hardware = true
        # Show WiFi setup step if wlan interfaces are found
        wifi_setup = ${if cfg.wifiSupport then "true" else "false"}
      '' + lib.optionalString (cfg.kanidmUrl != null) ''

        [kanidm]
        url = "${cfg.kanidmUrl}"
        client_id = "${cfg.kanidmClientId}"
      '';
      mode = "0644";
    };

    # --- Disko partition configs bundled in the enrollment image ---
    environment.etc."hearth/disko-configs/standard.nix" = {
      source = ../lib/disko-configs/standard.nix;
      mode = "0644";
    };
    environment.etc."hearth/disko-configs/luks-lvm.nix" = {
      source = ../lib/disko-configs/luks-lvm.nix;
      mode = "0644";
    };

    # --- Minimal package set ---
    environment.systemPackages = with pkgs; [
      cfg.package

      # Declarative disk partitioning
      disko

      # Disk utilities for partitioning and formatting (fallback + disko deps)
      gptfdisk    # sgdisk for GPT partitioning
      parted
      e2fsprogs   # mkfs.ext4
      dosfstools  # mkfs.fat
      cryptsetup
      lvm2        # LVM support for luks-lvm disko config
      nixos-install-tools

      # Network utilities
      iproute2
      iputils
      curl
      networkmanager

      # System utilities
      util-linux
      coreutils
      bash

      # Nix for installing the system
      nix

      # Hardware detection
      pciutils
      usbutils
      dmidecode
      lshw
    ];

    # --- Disable unnecessary services for a minimal enrollment image ---
    services.xserver.enable = false;
    security.polkit.enable = true;

    # --- Console configuration ---
    console = {
      font = "Lat2-Terminus16";
      keyMap = "us";
    };

    # --- Nix configuration for installing the target system ---
    nix.settings = {
      experimental-features = [ "nix-command" "flakes" ];
      # The enrollment image needs to pull closures from the Hearth cache
      trusted-users = [ "root" "enrollment" ];
      # The enrollment TUI writes /etc/nix/netrc at runtime with cache
      # credentials received from the control plane during approval.
      netrc-file = "/etc/nix/netrc";
    };

    # --- Boot configuration (for netboot/USB) ---
    # The enrollment image is designed to be used as a netboot ramdisk or USB image
    boot.loader.systemd-boot.enable = lib.mkDefault false;
    boot.loader.grub.enable = lib.mkDefault false;

    # Ensure enough tmpfs space for enrollment operations
    boot.tmp.useTmpfs = true;
    boot.tmp.tmpfsSize = "4G";
  };
}
