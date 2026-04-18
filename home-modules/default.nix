# home-modules/default.nix — Default/standard role home-manager profile
#
# The baseline user environment for users who don't match a specific role.
# Provides a clean, functional GNOME desktop with standard productivity
# tools configured.
{ config, lib, pkgs, ... }:

{
  imports = [ ./common.nix ];

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
      ] ++ lib.optionals config.hearth.libreoffice.enable [
        "org.libreoffice.LibreOffice.writer.desktop"
      ] ++ lib.optionals config.hearth.chat.enable [
        "org.gnome.Fractal.desktop"
      ] ++ lib.optionals config.hearth.nextcloud.enable [
        "com.nextcloud.desktopclient.nextcloud.desktop"
      ] ++ lib.optionals config.hearth.thunderbird.enable [
        "thunderbird.desktop"
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
  # LibreOffice is installed by home-modules/libreoffice.nix when enabled
  home.packages = with pkgs; [
    # Image viewer
    loupe

    # Calculator
    gnome-calculator

    # PDF viewer (evince is part of GNOME)
    evince
  ];
}
