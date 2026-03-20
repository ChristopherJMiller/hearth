# home-modules/default.nix — Default/standard role home-manager profile
#
# The baseline user environment for users who don't match a specific role.
# Provides a clean, functional GNOME desktop with standard productivity
# tools configured.
{ config, lib, pkgs, ... }:

{
  imports = [ ./common.nix ];

  # --- Firefox with managed policies ---
  programs.firefox = {
    enable = true;
    # Managed policies can be set here. In production, the control plane
    # provides per-org policy overrides.
    policies = {
      # Disable telemetry
      DisableTelemetry = true;
      # Disable studies
      DisableFirefoxStudies = true;
      # Disable pocket
      DisablePocket = true;
      # Enable tracking protection
      EnableTrackingProtection = {
        Value = true;
        Locked = false;
        Cryptomining = true;
        Fingerprinting = true;
      };
      # Default search engine
      SearchEngines = {
        Default = "DuckDuckGo";
      };
    };
  };

  # --- Nautilus bookmarks ---
  # Standard user bookmarks for the file manager
  xdg.configFile."gtk-3.0/bookmarks" = {
    text = ''
      file://${config.home.homeDirectory}/Documents Documents
      file://${config.home.homeDirectory}/Downloads Downloads
      file://${config.home.homeDirectory}/Pictures Pictures
    '' + lib.optionalString config.hearth.nextcloud.enable ''
      davs://${lib.removePrefix "https://" (lib.removePrefix "http://" config.hearth.nextcloud.serverUrl)}/remote.php/dav/files/${config.home.username}/ Cloud Storage
    '';
  };

  # --- GNOME settings for standard users ---
  dconf.settings = {
    "org/gnome/shell" = {
      favorite-apps = [
        "firefox.desktop"
        "org.gnome.Nautilus.desktop"
        "org.gnome.Terminal.desktop"
        "org.gnome.TextEditor.desktop"
        "org.libreoffice.LibreOffice.writer.desktop"
      ] ++ lib.optionals config.hearth.chat.enable [
        "element-desktop.desktop"
      ] ++ lib.optionals config.hearth.nextcloud.enable [
        "com.nextcloud.desktopclient.nextcloud.desktop"
      ];
    };

    "org/gnome/nautilus/preferences" = {
      default-folder-viewer = "list-view";
      show-hidden-files = false;
    };

    "org/gnome/nautilus/list-view" = {
      default-zoom-level = "small";
      use-tree-view = true;
    };

    "org/gnome/desktop/background" = {
      picture-options = "zoom";
      color-shading-type = "solid";
      primary-color = "#2c2c2c";
    };
  };

  # --- Additional packages for default role ---
  home.packages = with pkgs; [
    # Office suite
    libreoffice

    # Image viewer
    loupe

    # Calculator
    gnome-calculator

    # PDF viewer (evince is part of GNOME)
    evince
  ];
}
