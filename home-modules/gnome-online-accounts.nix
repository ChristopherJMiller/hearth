# home-modules/gnome-online-accounts.nix — GNOME Online Accounts integration
#
# Pre-seeds a Nextcloud account in GNOME Online Accounts so that:
# - GNOME Shell panel clock shows upcoming calendar events
# - GNOME Contacts displays CardDAV contacts from Nextcloud
#
# These feed via evolution-data-server, independent of Thunderbird.
# The user authenticates on first login; the account is pre-configured
# so they just confirm credentials rather than manually adding a provider.
#
# Enabled via hearth.gnomeOnlineAccounts.enable when the fleet has cloud capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.gnomeOnlineAccounts;
in
{
  options.hearth.gnomeOnlineAccounts = {
    enable = lib.mkEnableOption "GNOME Online Accounts (Nextcloud calendar/contacts)";

    nextcloudUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Ensure GNOME Contacts is available alongside the built-in GNOME Calendar
    home.packages = [ pkgs.gnome-contacts ];

    # --- Pre-seed Nextcloud account in GNOME Online Accounts ---
    # GOA reads accounts from this keyfile. The account is pre-configured
    # with calendar and contacts enabled; file sync is handled separately
    # by the Nextcloud Desktop client (home-modules/nextcloud.nix).
    xdg.configFile."goa-1.0/accounts.conf".text = ''
      [Account account_hearth_nextcloud]
      Provider=nextcloud
      Identity=${config.home.username}
      PresentationIdentity=${config.home.username}
      Uri=${cfg.nextcloudUrl}
      CalendarEnabled=true
      ContactsEnabled=true
      FilesEnabled=false
    '';

    # --- Ensure GNOME shell shows date + calendar events ---
    dconf.settings = {
      "org/gnome/desktop/interface" = {
        clock-show-date = true;
      };

      "org/gnome/desktop/calendar" = {
        show-weekdate = true;
      };
    };
  };
}
