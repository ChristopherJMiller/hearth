# home-modules/thunderbird.nix — Email, Calendar & Contacts (Thunderbird) home-manager module
#
# Pre-configures Thunderbird as a managed PIM client with Nextcloud
# CalDAV/CardDAV sync (via TbSync), optional mail server integration
# with Kanidm OIDC authentication, and managed extension policies.
#
# Enabled via hearth.thunderbird.enable when the fleet has cloud capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.thunderbird;

  # Build the extensions policy — always include CalDAV/CardDAV sync,
  # conditionally include the OIDC extension for mail auth
  extensionSettings = {
    # TbSync — CalDAV/CardDAV sync engine
    "tbsync@jobisoft.de" = {
      installation_mode = "force_installed";
      install_url = "https://addons.thunderbird.net/thunderbird/downloads/latest/tbsync/latest.xpi";
    };
    # DAV-4-TbSync — CalDAV & CardDAV provider for TbSync
    "dav4tbsync@jobisoft.de" = {
      installation_mode = "force_installed";
      install_url = "https://addons.thunderbird.net/thunderbird/downloads/latest/dav-4-tbsync/latest.xpi";
    };
  } // lib.optionalAttrs (cfg.mail.enable && cfg.mail.useOidc) {
    # Thunderbird Custom IDP — OAuth2/OIDC for IMAP/SMTP auth via Kanidm
    "custom-idp@niclas-arndt.de" = {
      installation_mode = "force_installed";
      install_url = "https://addons.thunderbird.net/thunderbird/downloads/latest/thunderbird-custom-idp/latest.xpi";
    };
  };

  # Mozilla autoconfig XML for mail account auto-setup
  autoconfigXml = ''
    <?xml version="1.0" encoding="UTF-8"?>
    <clientConfig version="1.1">
      <emailProvider id="hearth">
        <domain>${cfg.mail.domain}</domain>
        <displayName>Hearth Mail</displayName>
        <displayShortName>Hearth</displayShortName>
        <incomingServer type="imap">
          <hostname>${cfg.mail.imapHost}</hostname>
          <port>993</port>
          <socketType>SSL</socketType>
          <authentication>${if cfg.mail.useOidc then "OAuth2" else "password-cleartext"}</authentication>
          <username>%EMAILADDRESS%</username>
        </incomingServer>
        <outgoingServer type="smtp">
          <hostname>${cfg.mail.smtpHost}</hostname>
          <port>587</port>
          <socketType>STARTTLS</socketType>
          <authentication>${if cfg.mail.useOidc then "OAuth2" else "password-cleartext"}</authentication>
          <username>%EMAILADDRESS%</username>
        </outgoingServer>
      </emailProvider>
    </clientConfig>
  '';
in
{
  options.hearth.thunderbird = {
    enable = lib.mkEnableOption "Hearth email, calendar & contacts (Thunderbird)";

    nextcloudUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance (for CalDAV/CardDAV sync).";
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to auto-start Thunderbird on login.";
    };

    mail = {
      enable = lib.mkEnableOption "mail server integration";

      imapHost = lib.mkOption {
        type = lib.types.str;
        default = "";
        example = "mail.hearth.example.com";
        description = "IMAP server hostname.";
      };

      smtpHost = lib.mkOption {
        type = lib.types.str;
        default = "";
        example = "mail.hearth.example.com";
        description = "SMTP server hostname.";
      };

      domain = lib.mkOption {
        type = lib.types.str;
        default = "";
        example = "hearth.example.com";
        description = "Mail domain for user address derivation.";
      };

      useOidc = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Use Kanidm OIDC/OAuth2 for IMAP/SMTP authentication (eliminates stored passwords).";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    # --- Thunderbird package ---
    home.packages = [ pkgs.thunderbird ];

    # --- Managed policies (Mozilla policy engine) ---
    # Fleet-managed Thunderbird configuration: telemetry disabled, extensions
    # force-installed, CalDAV/CardDAV discovery URL pre-configured.
    xdg.configFile."thunderbird/policies/policies.json".text = builtins.toJSON {
      policies = {
        DisableTelemetry = true;
        DisableFirefoxStudies = true;
        ExtensionSettings = extensionSettings;
        Preferences = {
          # Disable telemetry
          "datareporting.policy.dataSubmissionEnabled" = { Value = false; Status = "locked"; };
          "toolkit.telemetry.enabled" = { Value = false; Status = "locked"; };
          # CalDAV/CardDAV auto-discovery hint for TbSync
          "extensions.tbsync.dav.server" = { Value = "${cfg.nextcloudUrl}/remote.php/dav"; Status = "default"; };
        };
      };
    };

    # --- Mail autoconfig XML (when mail is enabled) ---
    # Thunderbird looks for autoconfig files to pre-populate account setup.
    # This provides zero-touch mail configuration for fleet users.
    xdg.configFile."thunderbird/autoconfig.xml" = lib.mkIf cfg.mail.enable {
      text = autoconfigXml;
    };

    # --- Auto-start on GNOME login ---
    xdg.configFile."autostart/thunderbird.desktop" = lib.mkIf cfg.autoStart {
      text = ''
        [Desktop Entry]
        Type=Application
        Name=Hearth Mail & Calendar
        Comment=Email, calendar & contacts (Thunderbird)
        Exec=thunderbird
        Icon=thunderbird
        Terminal=false
        X-GNOME-Autostart-enabled=true
        X-GNOME-Autostart-Delay=5
      '';
    };
  };
}
