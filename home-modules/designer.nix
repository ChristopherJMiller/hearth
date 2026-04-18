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
    poppler-utils

    # Screenshot and screen recording
    gnome-screenshot

    # Fonts — extensive collection for design work.
    # google-fonts includes JetBrains Mono, so don't install it separately
    # to avoid buildEnv font file conflicts.
    google-fonts
    nerd-fonts.fira-code
    inter
    roboto
    lato
    source-code-pro
    source-sans
    source-serif
    liberation_ttf
    noto-fonts
    noto-fonts-cjk-sans
    noto-fonts-color-emoji
    font-awesome

    # LibreOffice is installed by home-modules/libreoffice.nix when enabled
  ];

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
