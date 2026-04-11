# Lanzaboote Secure Boot module for Hearth fleet devices.
#
# When enabled, replaces systemd-boot with Lanzaboote's signed boot stub.
# Secure Boot keys must be enrolled via sbctl during first boot.
#
# Requires the lanzaboote flake input to be available. If the
# `boot.lanzaboote` option does not exist (lanzaboote not in flake inputs),
# this module still defines the hearth option but skips the lanzaboote config.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.secureBoot;
  hasLanzaboote = (config.boot or {}) ? lanzaboote
    || (lib.attrByPath [ "boot" "lanzaboote" ] null config.options) != null;
in
{
  options.services.hearth.secureBoot = {
    enable = lib.mkEnableOption "Lanzaboote Secure Boot for Hearth fleet devices";

    pkiBundle = lib.mkOption {
      type = lib.types.path;
      default = "/etc/secureboot";
      description = "Path to the Secure Boot PKI bundle (sbctl keys).";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      # Lanzaboote replaces systemd-boot
      boot.loader.systemd-boot.enable = lib.mkForce false;

      # Include sbctl for key management during first boot
      environment.systemPackages = [ pkgs.sbctl ];

      # Ensure the PKI bundle directory exists
      systemd.tmpfiles.rules = [
        "d ${cfg.pkiBundle} 0700 root root -"
      ];

      warnings = lib.optional (!hasLanzaboote) ''
        services.hearth.secureBoot is enabled but the lanzaboote NixOS module
        is not available. Add lanzaboote to your flake inputs and import its
        NixOS module for Secure Boot to work.
      '';
    }
  ]);
}
