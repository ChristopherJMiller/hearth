# lib/mk-enrollment-image.nix — Builds a bootable ISO for Hearth device enrollment
#
# Produces a minimal NixOS ISO that boots into the hearth-enrollment TUI.
# Uses NixOS's installer ISO infrastructure for broad hardware compatibility.
#
# Usage from flake.nix:
#
#   packages.enrollment-iso = (import ./lib/mk-enrollment-image.nix {
#     inherit self nixpkgs system;
#     serverUrl = "https://api.hearth.example.com";
#   }).config.system.build.image;
#

{ self, nixpkgs, system ? "x86_64-linux", serverUrl ? "https://hearth.example.com", wifiSupport ? true, cacheUrl ? null, cachePublicKey ? null, kanidmUrl ? null, kanidmClientId ? "hearth-enrollment", kanidmCaCert ? null }:

let
  lib = nixpkgs.lib;
in
nixpkgs.lib.nixosSystem {
  modules = [
    # NixOS ISO image infrastructure — import iso-image.nix directly (not
    # installation-cd-*.nix) for maximum control over what goes in the ISO.
    "${nixpkgs}/nixos/modules/installer/cd-dvd/iso-image.nix"
    # Minimal profile: disables docs, default packages, logrotate, udisks2, XDG
    "${nixpkgs}/nixos/modules/profiles/minimal.nix"

    # Hearth overlay so pkgs.hearth-enrollment is available
    {
      nixpkgs.hostPlatform = system;
      nixpkgs.overlays = [
        self.overlays.default
      ];
    }

    # Hearth enrollment module
    ../modules/enrollment.nix

    # ISO and enrollment configuration
    ({ config, pkgs, lib, ... }: {
      # --- ISO image settings ---
      image.fileName = "hearth-enrollment-${config.system.nixos.label}.iso";
      isoImage = {
        volumeID = "HEARTH-ENROLL";
        # zstd for fast decompression on target hardware
        squashfsCompression = "zstd";
        # Make it a hybrid ISO (bootable from both USB and CD)
        makeEfiBootable = true;
        makeUsbBootable = true;
      };

      # --- Enable Hearth enrollment ---
      services.hearth.enrollment = {
        enable = true;
        inherit serverUrl wifiSupport;
      } // lib.optionalAttrs (kanidmUrl != null) {
        inherit kanidmUrl kanidmClientId;
      } // lib.optionalAttrs (kanidmCaCert != null) {
        inherit kanidmCaCert;
      };

      # --- System basics ---
      system.stateVersion = "25.05";

      # Boot — the ISO infrastructure handles the bootloader
      boot.loader.grub.enable = false;

      # --- Fast, silent boot ---
      # Skip boot menu (1 = 0.1s, effectively instant; iso-image.nix sets 10)
      boot.loader.timeout = lib.mkForce 1;
      # Suppress kernel/systemd boot messages — straight to the TUI
      boot.kernelParams = [ "quiet" "loglevel=3" "systemd.show_status=false" "rd.udev.log_level=3" ];
      boot.consoleLogLevel = 0;
      boot.initrd.verbose = false;

      # Only include filesystem support we actually need
      boot.supportedFilesystems = lib.mkForce [ "vfat" "ext4" "btrfs" ];

      # Strip pre-cached build dependencies (saves ~200MB)
      system.extraDependencies = lib.mkForce [];

      # Disable services that add boot time but aren't needed for enrollment
      services.speechd.enable = false;

      # Minimal fonts — just enough for browser rendering
      fonts.enableDefaultPackages = false;
      fonts.fontconfig.enable = true;
      fonts.packages = [ pkgs.dejavu_fonts ];

      # Nix — needed to install the target system (enrollment module also sets this)
      nix.settings = {
        experimental-features = lib.mkDefault [ "nix-command" "flakes" ];
        substituters = lib.mkForce (
          lib.optional (cacheUrl != null) cacheUrl
          ++ [ "https://cache.nixos.org" ]
        );
        trusted-public-keys = lib.optional (cachePublicKey != null) cachePublicKey
          ++ [ "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY=" ];
        trusted-users = [ "root" ];
        # Dev Attic caches may not have signing keys; allow unsigned paths.
        # Production builds should set cachePublicKey and remove this.
        require-sigs = cachePublicKey != null;
      };
    })
  ];
}
