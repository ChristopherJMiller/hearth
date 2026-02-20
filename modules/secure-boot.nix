# Lanzaboote Secure Boot module for Hearth fleet devices.
#
# When enabled, replaces systemd-boot with Lanzaboote's signed boot stub.
# Secure Boot keys must be enrolled via sbctl during first boot.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.secureBoot;
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

  config = lib.mkIf cfg.enable {
    # Lanzaboote replaces systemd-boot
    boot.loader.systemd-boot.enable = lib.mkForce false;

    boot.lanzaboote = {
      enable = true;
      pkiBundle = cfg.pkiBundle;
    };

    # Include sbctl for key management during first boot
    environment.systemPackages = [ pkgs.sbctl ];

    # Ensure the PKI bundle directory exists
    systemd.tmpfiles.rules = [
      "d ${cfg.pkiBundle} 0700 root root -"
    ];
  };
}
