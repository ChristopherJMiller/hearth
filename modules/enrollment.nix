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
        URL of the Kanidm server for OAuth2 authorization code flow.
        When set, the enrollment TUI will launch a kiosk browser for
        user authentication before device enrollment.
      '';
    };

    kanidmClientId = lib.mkOption {
      type = lib.types.str;
      default = "hearth-enrollment";
      description = "OAuth2 client ID for the enrollment authorization code flow.";
    };

    kanidmCaCert = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to a CA certificate (PEM) to trust for the Kanidm server.
        Use this for self-signed dev certs so the kiosk browser accepts
        the TLS connection without user interaction.
      '';
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
      # Resolve *.hearth.local hostnames to the QEMU host gateway so the
      # enrollment VM can reach dev services running on the host.
      hosts = {
        "10.0.2.2" = [ "api.hearth.local" "cache.hearth.local" ]
          ++ lib.optionals (cfg.kanidmUrl != null) [ "kanidm.hearth.local" ];
      };
    };

    # Only include redistributable firmware when WiFi is needed — this avoids
    # pulling in the full linux-firmware blob (~500MB) for wired-only enrollment.
    hardware.enableRedistributableFirmware = lib.mkIf cfg.wifiSupport true;
    # Broad hardware support for varied enrollment targets (NVMe, USB, SATA, etc.)
    hardware.enableAllHardware = lib.mkDefault true;

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

    # --- Seat management for kiosk browser (cage needs DRM access) ---
    services.seatd.enable = lib.mkIf (cfg.kanidmUrl != null) true;
    users.users.root.extraGroups = lib.mkIf (cfg.kanidmUrl != null) [ "seat" ];

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

      # System utilities
      util-linux

      # Nix for installing the system
      nix

      # Hardware detection
      pciutils
      usbutils
      dmidecode
      lshw
    ]
    # Kiosk browser only needed when Kanidm OAuth2 is configured
    ++ lib.optionals (cfg.kanidmUrl != null) [
      cage          # Wayland kiosk compositor
      firefox       # Web browser for Kanidm login
      mesa.drivers  # GPU/software rendering for Wayland
    ]
    # NetworkManager only needed for WiFi
    ++ lib.optionals cfg.wifiSupport [
      networkmanager
    ];

    # --- Trust custom CA cert for Kanidm (e.g. self-signed dev certs) ---
    security.pki.certificateFiles = lib.mkIf (cfg.kanidmCaCert != null) [
      cfg.kanidmCaCert
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
