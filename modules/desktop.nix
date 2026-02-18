# modules/desktop.nix — NixOS module for GNOME desktop baseline
#
# Sets up a standardized GNOME desktop environment for fleet machines.
# Uses greetd (not GDM) as the display manager, enables PipeWire for audio,
# configures Flatpak with Flathub, and sets organization-wide GNOME defaults.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.desktop;

  # Build a dconf database overlay for system-wide GNOME defaults
  hearthDconfDb = pkgs.writeTextFile {
    name = "hearth-dconf-defaults";
    destination = "/etc/dconf/db/hearth.d/00-hearth-defaults";
    text = ''
      # Hearth fleet-wide GNOME defaults
      # Users can override these in their own dconf settings

      [org/gnome/desktop/interface]
      color-scheme='prefer-dark'
      gtk-theme='Adwaita-dark'
      font-name='Cantarell 11'
      document-font-name='Cantarell 11'
      monospace-font-name='Source Code Pro 10'
      clock-show-weekday=true
      clock-show-seconds=false
      enable-hot-corners=false

      [org/gnome/desktop/peripherals/touchpad]
      tap-to-click=true
      natural-scroll=true

      [org/gnome/desktop/privacy]
      remember-recent-files=true
      recent-files-max-age=30
      remove-old-trash-files=true
      remove-old-temp-files=true
      old-files-age=30

      [org/gnome/desktop/screensaver]
      lock-enabled=true
      lock-delay=uint32 300

      [org/gnome/desktop/session]
      idle-delay=uint32 600

      [org/gnome/desktop/wm/preferences]
      button-layout='appmenu:minimize,maximize,close'

      [org/gnome/settings-daemon/plugins/power]
      sleep-inactive-ac-type='nothing'
      sleep-inactive-battery-timeout=900

      [org/gnome/shell]
      favorite-apps=['org.gnome.Nautilus.desktop', 'firefox.desktop', 'org.gnome.Terminal.desktop', 'org.gnome.TextEditor.desktop']
    '';
  };

  # dconf profile that layers hearth defaults under user settings
  dconfProfile = pkgs.writeTextFile {
    name = "hearth-dconf-profile";
    destination = "/etc/dconf/profile/user";
    text = ''
      user-db:user
      system-db:hearth
    '';
  };
in
{
  options.services.hearth.desktop = {
    enable = lib.mkEnableOption "Hearth GNOME desktop baseline";

    enableFlatpak = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to enable Flatpak with the Flathub remote.";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      description = "Additional packages to install on all desktop machines.";
    };

    wallpaper = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Path to a custom wallpaper image for the default desktop background.";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- GNOME Desktop Environment ---
    services.xserver = {
      enable = true;
      # We use greetd, not GDM
      displayManager.gdm.enable = lib.mkForce false;
      desktopManager.gnome.enable = true;
    };

    # Ensure Wayland session is available
    programs.xwayland.enable = true;

    # --- Audio via PipeWire ---
    # Disable PulseAudio in favor of PipeWire
    hardware.pulseaudio.enable = lib.mkForce false;
    services.pipewire = {
      enable = true;
      alsa.enable = true;
      alsa.support32Bit = true;
      pulse.enable = true;
      wireplumber.enable = true;
    };
    # rtkit for realtime audio scheduling
    security.rtkit.enable = true;

    # --- Flatpak ---
    services.flatpak.enable = lib.mkIf cfg.enableFlatpak true;
    # Flathub remote is added via activation script since there's no
    # declarative option for flatpak remotes in NixOS
    system.activationScripts.flatpak-flathub = lib.mkIf cfg.enableFlatpak {
      text = ''
        ${pkgs.flatpak}/bin/flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo || true
      '';
    };

    # --- dconf system defaults ---
    # Install the dconf database overlay and profile
    environment.etc = {
      "dconf/db/hearth.d/00-hearth-defaults" = {
        source = "${hearthDconfDb}/etc/dconf/db/hearth.d/00-hearth-defaults";
        mode = "0644";
      };
      "dconf/profile/user" = {
        source = "${dconfProfile}/etc/dconf/profile/user";
        mode = "0644";
      };
    };

    # Compile the dconf database on activation
    system.activationScripts.dconf-compile = {
      text = ''
        ${pkgs.dconf}/bin/dconf compile /etc/dconf/db/hearth /etc/dconf/db/hearth.d || true
      '';
      deps = [ ];
    };

    # --- Base desktop packages ---
    environment.systemPackages = with pkgs; [
      # Web browser
      firefox

      # Text editor
      gnome-text-editor

      # File manager (already part of GNOME, but ensure it's present)
      nautilus

      # Terminal
      gnome-terminal

      # System utilities
      gnome-system-monitor
      gnome-tweaks
      file-roller

      # Fonts
      noto-fonts
      noto-fonts-cjk-sans
      noto-fonts-emoji
      liberation_ttf
      source-code-pro
    ] ++ cfg.extraPackages;

    # --- Font configuration ---
    fonts.fontconfig = {
      enable = true;
      defaultFonts = {
        sansSerif = [ "Cantarell" "Noto Sans" ];
        serif = [ "Noto Serif" ];
        monospace = [ "Source Code Pro" "Noto Sans Mono" ];
        emoji = [ "Noto Color Emoji" ];
      };
    };

    # --- XDG Portal for Flatpak and screen sharing ---
    xdg.portal = {
      enable = true;
      # xdg-desktop-portal-gnome is pulled in by GNOME
    };

    # --- Printing support (basic CUPS) ---
    services.printing.enable = true;

    # --- Power management ---
    services.power-profiles-daemon.enable = true;

    # --- Disable GNOME packages we don't need on managed systems ---
    environment.gnome.excludePackages = with pkgs; [
      epiphany        # we use Firefox
      gnome-music     # not needed for enterprise
      gnome-tour      # first-run tour is replaced by our onboarding
      totem           # video player not needed by default
    ];
  };
}
