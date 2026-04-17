# modules/firefox.nix — NixOS module for Hearth managed Firefox on fleet devices
#
# Provides the option interface for mk-fleet-host.nix to wire Firefox
# configuration. Per-user configuration (policies, managed bookmarks,
# extensions, homepage) is handled by the home-manager module
# (home-modules/firefox.nix).
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.firefox;

  bookmarkType = lib.types.submodule {
    options = {
      name = lib.mkOption { type = lib.types.str; description = "Display name for the bookmark."; };
      url = lib.mkOption { type = lib.types.str; description = "URL of the service."; };
    };
  };
in
{
  options.services.hearth.firefox = {
    enable = lib.mkEnableOption "Hearth managed Firefox browser";

    consoleUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://hearth.example.com";
      description = "URL of the Hearth console (used as Firefox homepage).";
    };

    services = lib.mkOption {
      type = lib.types.listOf bookmarkType;
      default = [];
      description = "Platform services to add as managed bookmarks in Firefox.";
    };

    nextcloudUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Nextcloud URL for Floccus bookmark sync. When set, Floccus extension is force-installed and pre-configured.";
    };

    vaultwardenUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Vaultwarden URL. When set, Bitwarden extension is force-installed and pre-configured.";
    };

    internalCaCerts = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [];
      description = "Internal CA certificates to trust in Firefox.";
    };

    extraBookmarks = lib.mkOption {
      type = lib.types.listOf bookmarkType;
      default = [];
      description = "Additional organisation-specific bookmarks.";
    };

    role = lib.mkOption {
      type = lib.types.str;
      default = "default";
      description = "Current machine role. Used to select role-specific bookmarks.";
    };

    roleBookmarks = lib.mkOption {
      type = lib.types.attrsOf (lib.types.listOf bookmarkType);
      default = {};
      description = "Role-specific bookmarks. Keys are role names, values are bookmark lists.";
    };

    dnsOverHttps = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Enable DNS-over-HTTPS in Firefox.";
      };

      providerUrl = lib.mkOption {
        type = lib.types.str;
        default = "";
        description = "DoH resolver URL (e.g., fleet resolver or public provider).";
      };
    };
  };

  # No system-level config needed — Firefox is installed and configured
  # per-user by home-modules/firefox.nix via home-manager.
}
