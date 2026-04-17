# home-modules/libreoffice.nix — Managed LibreOffice home-manager module
#
# Pre-configures LibreOffice with fleet-managed defaults: Nextcloud WebDAV
# integration, default save/template paths, enterprise fonts, macro security,
# and disabled telemetry. Supports installing .oxt extensions (Phase 2:
# Hearth Rust UNO extensions for Nextcloud share/comments/lock status).
#
# Enabled via hearth.libreoffice.enable when the fleet has cloud capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.libreoffice;

  # Strip protocol prefix for WebDAV URL construction
  serverHost = lib.removePrefix "https://" (lib.removePrefix "http://" cfg.nextcloudUrl);

  # Nextcloud WebDAV base URL for remote file access
  webdavUrl = "${cfg.nextcloudUrl}/remote.php/dav/files/";

  # Macro security level: 3 = high (only signed macros), 4 = very high (all disabled)
  macroSecurityLevel = if cfg.macroSecurity == "very-high" then 4 else 3;

  # LibreOffice uses XML-based registry modifications for user preferences.
  # Each <item> sets a configuration property via its oor:path.
  registryModifications = ''
    <?xml version="1.0" encoding="UTF-8"?>
    <oor:items xmlns:oor="http://openoffice.org/2001/registry"
               xmlns:xs="http://www.w3.org/2001/XMLSchema"
               xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">

      <!-- Default working directory → ~/Nextcloud/Documents/ -->
      <item oor:path="/org.openoffice.Office.Paths/Paths/org.openoffice.Office.Paths:NamedPath['Work']/WritePath">
        <value>${cfg.defaultSaveDir}</value>
      </item>

      <!-- Template directory → ~/Nextcloud/Templates/ -->
      <item oor:path="/org.openoffice.Office.Paths/Paths/org.openoffice.Office.Paths:NamedPath['Template']/WritePath">
        <value>${cfg.templateDir}</value>
      </item>

      <!-- File locking (enables WebDAV LOCK awareness) -->
      <item oor:path="/org.openoffice.Office.Common/Misc/UseLocking">
        <value>true</value>
      </item>

      <!-- Macro security level -->
      <item oor:path="/org.openoffice.Office.Common/Security/Scripting/MacroSecurityLevel">
        <value>${toString macroSecurityLevel}</value>
      </item>

      <!-- Disable crash reporting -->
      <item oor:path="/org.openoffice.Office.Common/Misc/CrashReport">
        <value>false</value>
      </item>

      <!-- Disable usage statistics -->
      <item oor:path="/org.openoffice.Office.Common/Misc/CollectUsageInformation">
        <value>false</value>
      </item>

      <!-- Default sans-serif font -->
      <item oor:path="/org.openoffice.VCL/DefaultFonts/SANS">
        <value>${cfg.defaultFonts.sans}</value>
      </item>

      <!-- Default serif font -->
      <item oor:path="/org.openoffice.VCL/DefaultFonts/SERIF">
        <value>${cfg.defaultFonts.serif}</value>
      </item>

      <!-- Default monospace font -->
      <item oor:path="/org.openoffice.VCL/DefaultFonts/FIXED">
        <value>${cfg.defaultFonts.mono}</value>
      </item>

      <!-- Use new-style file picker (GTK) for better GVFS/WebDAV integration -->
      <item oor:path="/org.openoffice.Office.Common/Misc/UseSystemFileDialog">
        <value>true</value>
      </item>

    </oor:items>
  '';

  # Nextcloud remote server entry for File → Open Remote
  # LibreOffice stores remote servers in a separate config file
  remoteServersXml = ''
    <?xml version="1.0" encoding="UTF-8"?>
    <ServerList>
      <Server>
        <Name>Hearth Cloud Storage</Name>
        <Url>${webdavUrl}</Url>
        <Type>WEBDAV</Type>
      </Server>
    </ServerList>
  '';

  # Extension config for the Rust UNO extensions (Phase 2)
  officeConfig = ''
    # Hearth LibreOffice extension configuration
    # Written by home-manager — do not edit manually

    [nextcloud]
    url = "${cfg.nextcloudUrl}"
    webdav_url = "${webdavUrl}"
  '';

in
{
  options.hearth.libreoffice = {
    enable = lib.mkEnableOption "Hearth managed LibreOffice";

    nextcloudUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance for WebDAV, share API, and comments.";
    };

    defaultSaveDir = lib.mkOption {
      type = lib.types.str;
      default = "${config.home.homeDirectory}/Nextcloud/Documents";
      description = "Default save directory for new documents.";
    };

    templateDir = lib.mkOption {
      type = lib.types.str;
      default = "${config.home.homeDirectory}/Nextcloud/Templates";
      description = "Directory for document templates.";
    };

    defaultFonts = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {
        sans = "DM Sans";
        serif = "Noto Serif";
        mono = "JetBrains Mono";
      };
      description = "Default font families for documents. Keys: sans, serif, mono.";
    };

    macroSecurity = lib.mkOption {
      type = lib.types.enum [ "high" "very-high" ];
      default = "high";
      description = ''
        Macro security level. "high" allows only signed macros from trusted
        sources. "very-high" disables all macros unconditionally.
      '';
    };

    extensions = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [];
      description = "List of .oxt extension packages to install (e.g., hearth-office-oxt).";
    };

    enableExtensions = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Install Hearth LibreOffice extensions (Nextcloud Share, Comments
        sidebar, Lock Status). Requires LibreOffice built with Rust UNO support.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # --- LibreOffice package + fonts ---
    home.packages = with pkgs; [
      libreoffice

      # Hearth design system fonts for consistent document formatting
      dm-sans
      jetbrains-mono
      noto-fonts
    ];

    # --- Managed LibreOffice preferences ---
    # registrymodifications.xcu is the primary user preferences file. Writing
    # it declaratively enforces fleet-wide consistency. User changes made in
    # LO's Tools → Options dialog will be restored on the next home-manager
    # activation — this is the intended behavior for fleet management.
    xdg.configFile."libreoffice/4/user/registrymodifications.xcu".text = registryModifications;

    # --- Pre-configured Nextcloud remote server ---
    # Appears in File → Open Remote Document without manual setup.
    xdg.configFile."libreoffice/4/user/remote-servers.xml".text = remoteServersXml;

    # --- Extension configuration file ---
    # Read by the Hearth Rust UNO extensions (Phase 2) for Nextcloud API access.
    xdg.configFile."hearth/office.toml".text = officeConfig;

    # --- Create default directories ---
    # Ensure ~/Nextcloud/Documents and ~/Nextcloud/Templates exist so LO's
    # configured save/template paths work on first launch.
    home.activation.hearthLibreOfficeDirs = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      mkdir -p "${cfg.defaultSaveDir}"
      mkdir -p "${cfg.templateDir}"
    '';

    # --- Extension installation ---
    # Install .oxt extensions via unopkg on home-manager activation.
    # Extensions are placed in the user's extension directory and registered
    # with LibreOffice's extension manager.
    home.activation.hearthLibreOfficeExtensions = lib.mkIf (cfg.extensions != []) (
      lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        UNOPKG="${pkgs.libreoffice}/lib/libreoffice/program/unopkg"
        if [ -x "$UNOPKG" ]; then
          for ext in ${lib.concatMapStringsSep " " (e: "${e}/*.oxt") cfg.extensions}; do
            if [ -f "$ext" ]; then
              $UNOPKG add --suppress-license "$ext" 2>/dev/null || true
            fi
          done
        fi
      ''
    );
  };
}
