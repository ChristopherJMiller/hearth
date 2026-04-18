# home-modules/nextcloud.nix — Cloud storage (Nextcloud) home-manager module
#
# Pre-configures the Nextcloud Desktop sync client with the Hearth Nextcloud
# server, adds WebDAV bookmarks to Nautilus for online file browsing, and
# sets up a systemd user service for persistent GVFS WebDAV mount on login.
#
# Enabled via hearth.nextcloud.enable when the fleet has cloud capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.nextcloud;

  # Extract host from serverUrl for WebDAV path construction
  # e.g., "https://cloud.hearth.example.com" → WebDAV base path
  webdavUrl = "${cfg.serverUrl}/remote.php/dav/files/${config.home.username}/";
in
{
  options.hearth.nextcloud = {
    enable = lib.mkEnableOption "Hearth cloud storage (Nextcloud Desktop)";

    serverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance.";
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to auto-start Nextcloud Desktop sync client on login.";
    };

    mountWebdav = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to mount WebDAV storage via GVFS on login for Nautilus access.";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- Nextcloud Desktop sync client ---
    home.packages = with pkgs; [
      nextcloud-client
      # LibreOffice works with synced files in ~/Nextcloud and can open
      # davs:// URLs natively when GVFS is enabled — no extra config needed
    ];

    # --- Pre-configure Nextcloud Desktop client ---
    # Points at the Hearth Nextcloud instance. On first launch, the client
    # will open a browser for OIDC login via Kanidm.
    # Nextcloud Desktop config uses QSettings INI format with backslash-delimited
    # array keys. authType=webflow triggers browser-based OIDC login via Kanidm.
    # Keys: url, authType, version (from Nextcloud Desktop source: accountmanager.cpp)
    xdg.configFile."Nextcloud/nextcloud.cfg".text = ''
[General]
optionalServerNotifications=true

[Accounts]
version=2
0\url=${cfg.serverUrl}
0\authType=webflow
0\version=1
    '';

    # --- Auto-start Nextcloud sync client on GNOME login ---
    xdg.configFile."autostart/com.nextcloud.desktopclient.nextcloud.desktop" = lib.mkIf cfg.autoStart {
      text = ''
        [Desktop Entry]
        Type=Application
        Name=Hearth Cloud Storage
        Comment=Nextcloud Desktop sync client
        Exec=nextcloud --background
        Icon=Nextcloud
        Terminal=false
        X-GNOME-Autostart-enabled=true
        X-GNOME-Autostart-Delay=5
      '';
    };

    # --- GVFS WebDAV mount on login ---
    # Mounts the user's Nextcloud storage via GVFS so it appears in Nautilus
    # as a network location. This provides online browsing of all server-side
    # files without syncing everything locally.
    systemd.user.services.hearth-nextcloud-mount = lib.mkIf cfg.mountWebdav {
      Unit = {
        Description = "Mount Hearth Cloud Storage via WebDAV";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        Type = "oneshot";
        RemainAfterExit = true;
        # gio mount triggers GVFS to mount the WebDAV share; if auth is
        # needed it will use the GNOME Keyring credentials stored by the
        # Nextcloud Desktop client's OIDC login.
        ExecStart = "${pkgs.glib}/bin/gio mount ${webdavUrl}";
        ExecStop = "${pkgs.glib}/bin/gio mount -u ${webdavUrl}";
        # Don't fail the service if mount fails (e.g., offline)
        SuccessExitStatus = "0 1 2";
      };
      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };
  };
}
