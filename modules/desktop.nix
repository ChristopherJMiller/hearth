# modules/desktop.nix — NixOS module for GNOME desktop baseline
#
# Sets up a standardized GNOME desktop environment for fleet machines.
# Uses greetd (not GDM) as the display manager, enables PipeWire for audio,
# configures Flatpak with Flathub, and sets organization-wide GNOME defaults.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.desktop;

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
    services.xserver.enable = true;
    # We use greetd, not GDM
    services.displayManager.gdm.enable = lib.mkForce false;
    services.desktopManager.gnome.enable = true;

    # Ensure Wayland session is available
    programs.xwayland.enable = true;

    # --- Audio via PipeWire ---
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
    # User-level dconf settings (dark theme, touchpad, privacy, etc.) are
    # managed by home-manager via the role profiles in home-modules/.
    # The system module only ensures GNOME + dconf are available.
    programs.dconf.enable = true;

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

      # GNOME extensions
      gnomeExtensions.dash-to-panel

      # Fonts
      noto-fonts
      noto-fonts-cjk-sans
      noto-fonts-color-emoji
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
