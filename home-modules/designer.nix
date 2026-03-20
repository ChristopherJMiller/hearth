# home-modules/designer.nix — Designer role home-manager profile
#
# Environment for designers and creative professionals. Includes graphic
# design tools, rich font collections, color management, and GNOME settings
# tuned for visual work.
{ config, lib, pkgs, ... }:

{
  imports = [ ./common.nix ];

  # --- GNOME settings for design work ---
  dconf.settings = {
    "org/gnome/shell" = {
      favorite-apps = [
        "firefox.desktop"
        "gimp.desktop"
        "org.inkscape.Inkscape.desktop"
        "org.gnome.Nautilus.desktop"
        "org.gnome.TextEditor.desktop"
      ] ++ lib.optionals config.hearth.chat.enable [
        "element-desktop.desktop"
      ] ++ lib.optionals config.hearth.nextcloud.enable [
        "com.nextcloud.desktopclient.nextcloud.desktop"
      ];
    };

    "org/gnome/desktop/interface" = {
      # Larger text for design review comfort
      text-scaling-factor = 1.0;
      # Ensure subpixel rendering for font accuracy
      font-antialiasing = "rgba";
      font-hinting = "slight";
    };

    "org/gnome/desktop/background" = {
      picture-options = "zoom";
      color-shading-type = "solid";
      # Neutral gray background for color-accurate work
      primary-color = "#404040";
    };

    # Color management
    "org/gnome/settings-daemon/plugins/color" = {
      night-light-enabled = false; # Designers need accurate colors
    };

    # File manager set to icon view for visual browsing
    "org/gnome/nautilus/preferences" = {
      default-folder-viewer = "icon-view";
      show-image-thumbnails = "always";
    };

    "org/gnome/nautilus/icon-view" = {
      default-zoom-level = "large";
    };
  };

  # --- Nautilus bookmarks for design workflow ---
  xdg.configFile."gtk-3.0/bookmarks" = {
    text = ''
      file://${config.home.homeDirectory}/Documents Documents
      file://${config.home.homeDirectory}/Downloads Downloads
      file://${config.home.homeDirectory}/Pictures Pictures
      file://${config.home.homeDirectory}/Projects Projects
      file://${config.home.homeDirectory}/Assets Assets
    '' + lib.optionalString config.hearth.nextcloud.enable ''
      davs://${lib.removePrefix "https://" (lib.removePrefix "http://" config.hearth.nextcloud.serverUrl)}/remote.php/dav/files/${config.home.username}/ Cloud Storage
    '';
  };

  # Create standard design directories
  home.activation.designDirs = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    mkdir -p "${config.home.homeDirectory}/Projects"
    mkdir -p "${config.home.homeDirectory}/Assets"
    mkdir -p "${config.home.homeDirectory}/Assets/Fonts"
    mkdir -p "${config.home.homeDirectory}/Assets/Icons"
    mkdir -p "${config.home.homeDirectory}/Assets/Templates"
  '';

  # --- Designer packages ---
  home.packages = with pkgs; [
    # Graphic design
    gimp
    inkscape
    krita

    # Vector and diagram tools
    drawio

    # Font management
    fontforge
    gnome-font-viewer

    # Color tools
    colord
    gpick

    # Image viewers and converters
    loupe
    imagemagick
    optipng

    # PDF tools
    evince
    poppler_utils

    # Screenshot and screen recording
    gnome-screenshot

    # Fonts — extensive collection for design work
    google-fonts
    nerd-fonts.fira-code
    nerd-fonts.jetbrains-mono
    inter
    roboto
    lato
    source-code-pro
    source-sans
    source-serif
    liberation_ttf
    noto-fonts
    noto-fonts-cjk-sans
    noto-fonts-emoji
    font-awesome

    # Office suite for presentations
    libreoffice
  ];

  # --- Firefox configured for design reference ---
  programs.firefox = {
    enable = true;
    policies = {
      DisableTelemetry = true;
      DisableFirefoxStudies = true;
    };
  };

  # --- Shell with design-specific aliases ---
  programs.bash = {
    shellAliases = {
      # Quick image operations
      resize = "magick mogrify -resize";
      topng = "magick mogrify -format png";
      tojpg = "magick mogrify -format jpg -quality 90";
      # Open in default viewer
      view = "xdg-open";
    };
  };
}
