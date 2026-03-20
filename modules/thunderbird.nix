# modules/thunderbird.nix — NixOS module for Hearth email/calendar/contacts on fleet devices
#
# Provides the option interface for mk-fleet-host.nix to wire Thunderbird
# configuration. Per-user configuration (policies, extensions, autostart) is
# handled by the home-manager module (home-modules/thunderbird.nix).
#
# This module ensures system-level dependencies for GNOME calendar/contacts
# integration are available (evolution-data-server, GNOME Online Accounts).
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.thunderbird;
in
{
  options.services.hearth.thunderbird = {
    enable = lib.mkEnableOption "Hearth email, calendar & contacts (Thunderbird)";

    nextcloudUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance (for CalDAV/CardDAV).";
    };

    mail = {
      enable = lib.mkEnableOption "mail server integration";

      imapHost = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "IMAP server hostname.";
      };

      smtpHost = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "SMTP server hostname.";
      };

      domain = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Mail domain.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    # GNOME Online Accounts and evolution-data-server provide calendar/contacts
    # integration for the GNOME shell panel clock and GNOME Contacts app.
    environment.systemPackages = with pkgs; [
      gnome-online-accounts
      evolution-data-server
    ];
  };
}
