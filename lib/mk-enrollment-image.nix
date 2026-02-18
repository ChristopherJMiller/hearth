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
#   }).config.system.build.isoImage;
#

{ self, nixpkgs, system ? "x86_64-linux", serverUrl ? "https://hearth.example.com", wifiSupport ? true }:

let
  lib = nixpkgs.lib;
in
nixpkgs.lib.nixosSystem {
  inherit system;

  modules = [
    # NixOS ISO image infrastructure
    "${nixpkgs}/nixos/modules/installer/cd-dvd/iso-image.nix"
    "${nixpkgs}/nixos/modules/profiles/all-hardware.nix"

    # Hearth overlay so pkgs.hearth-enrollment is available
    {
      nixpkgs.overlays = [
        self.overlays.default
      ];
    }

    # Hearth enrollment module
    ../modules/enrollment.nix

    # ISO and enrollment configuration
    ({ config, pkgs, lib, ... }: {
      # --- ISO image settings ---
      isoImage = {
        isoName = "hearth-enrollment-${config.system.nixos.label}.iso";
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
      };

      # --- System basics ---
      system.stateVersion = "25.05";

      # Boot — the ISO infrastructure handles the bootloader
      boot.loader.grub.enable = false;

      # Nix — needed to install the target system (enrollment module also sets this)
      nix.settings.experimental-features = lib.mkDefault [ "nix-command" "flakes" ];
    })
  ];
}
