# home-modules/firefox.nix — Managed Firefox home-manager module
#
# Deploys Firefox with enterprise policies: managed bookmarks for platform
# services (including per-role bookmarks), force-installed extensions
# (uBlock Origin, Floccus for bookmark sync, Bitwarden for password
# management), homepage set to the Hearth console, internal CA certificate
# trust, and optional DNS-over-HTTPS.
#
# Enabled via hearth.firefox.enable; wired from mk-fleet-host.nix based on
# which capabilities are active for the fleet.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.firefox;

  # Bookmark submodule type (reused for services, extraBookmarks, roleBookmarks)
  bookmarkType = lib.types.submodule {
    options = {
      name = lib.mkOption { type = lib.types.str; description = "Display name for the bookmark."; };
      url = lib.mkOption { type = lib.types.str; description = "URL of the service."; };
    };
  };

  # Build the managed bookmarks list from enabled services + role + extras
  roleBookmarksList = cfg.roleBookmarks.${cfg.role} or [];

  managedBookmarks = [
    { toplevel_name = "Hearth"; }
  ] ++ map (svc: { url = svc.url; name = svc.name; }) cfg.services
    ++ cfg.extraBookmarks
    ++ roleBookmarksList;

  # Extension policies — uBlock Origin always; Floccus when Nextcloud available;
  # Bitwarden when Vaultwarden available
  extensionSettings = {
    # uBlock Origin — ad/tracker blocking
    "uBlock0@raymondhill.net" = {
      installation_mode = "force_installed";
      install_url = "https://addons.mozilla.org/firefox/downloads/latest/ublock-origin/latest.xpi";
    };
  } // lib.optionalAttrs (cfg.nextcloudUrl != null) {
    # Floccus — bookmark sync via Nextcloud Bookmarks
    "floccus@niclas-arndt.de" = {
      installation_mode = "force_installed";
      install_url = "https://addons.mozilla.org/firefox/downloads/latest/floccus/latest.xpi";
    };
  } // lib.optionalAttrs (cfg.vaultwardenUrl != null) {
    # Bitwarden — password manager (pre-configured for self-hosted Vaultwarden)
    "{446900e4-71c2-419f-a6a7-df9c091e268b}" = {
      installation_mode = "force_installed";
      install_url = "https://addons.mozilla.org/firefox/downloads/latest/bitwarden-password-manager/latest.xpi";
    };
  };

  # 3rdparty extension configuration (managed_storage) for pre-seeding
  # extension settings without user interaction
  thirdPartyExtensions =
    lib.optionalAttrs (cfg.nextcloudUrl != null) {
      # Floccus: pre-seed Nextcloud Bookmarks account
      "floccus@niclas-arndt.de" = {
        managedAccounts = [{
          type = "nextcloud-bookmarks";
          url = cfg.nextcloudUrl;
          label = "Hearth Bookmarks";
        }];
      };
    } // lib.optionalAttrs (cfg.vaultwardenUrl != null) {
      # Bitwarden: point at self-hosted Vaultwarden instance
      "{446900e4-71c2-419f-a6a7-df9c091e268b}" = {
        environment.base = cfg.vaultwardenUrl;
      };
    };
in
{
  options.hearth.firefox = {
    enable = lib.mkEnableOption "Hearth managed Firefox browser";

    consoleUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://hearth.example.com";
      description = "URL of the Hearth console (used as Firefox homepage).";
    };

    services = lib.mkOption {
      type = lib.types.listOf bookmarkType;
      default = [];
      description = "Platform services to add as managed bookmarks.";
    };

    nextcloudUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "https://cloud.hearth.example.com";
      description = "Nextcloud URL. When set, Floccus extension is force-installed and pre-configured for bookmark sync.";
    };

    vaultwardenUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "https://vault.hearth.example.com";
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
      description = "Additional organisation-specific bookmarks to include in the managed folder.";
    };

    role = lib.mkOption {
      type = lib.types.str;
      default = "default";
      description = "Current machine role. Used to select role-specific bookmarks.";
    };

    roleBookmarks = lib.mkOption {
      type = lib.types.attrsOf (lib.types.listOf bookmarkType);
      default = {};
      example = {
        developer = [{ name = "CI/CD"; url = "https://ci.example.com"; }];
        admin = [{ name = "Grafana"; url = "https://grafana.example.com"; }];
      };
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
        example = "https://dns.hearth.example.com/dns-query";
        description = "DoH resolver URL (e.g., fleet resolver or public provider).";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    programs.firefox = {
      enable = true;
      policies = {
        # --- Privacy & telemetry ---
        DisableTelemetry = true;
        DisableFirefoxStudies = true;
        DisablePocket = true;
        EnableTrackingProtection = {
          Value = true;
          Locked = false;
          Cryptomining = true;
          Fingerprinting = true;
        };

        # --- Homepage ---
        Homepage = {
          URL = cfg.consoleUrl;
          Locked = false;
          StartPage = "homepage";
        };

        # --- Search ---
        SearchEngines = {
          Default = "DuckDuckGo";
        };

        # --- Managed bookmarks (read-only, appear in "Hearth" folder) ---
        ManagedBookmarks = managedBookmarks;

        # --- Extensions ---
        ExtensionSettings = extensionSettings;

        # --- Extension pre-configuration via managed_storage ---
        "3rdparty".Extensions = lib.mkIf (thirdPartyExtensions != {}) thirdPartyExtensions;

        # --- Certificate trust ---
        Certificates = lib.mkIf (cfg.internalCaCerts != []) {
          Install = cfg.internalCaCerts;
        };

        # --- DNS-over-HTTPS ---
        DNSOverHTTPS = lib.mkIf cfg.dnsOverHttps.enable {
          Enabled = true;
          Locked = true;
          ProviderURL = cfg.dnsOverHttps.providerUrl;
        };
      };
    };
  };
}
