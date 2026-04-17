# modules/libreoffice.nix — NixOS module for Hearth managed LibreOffice on fleet devices
#
# Provides the option interface for mk-fleet-host.nix to wire LibreOffice
# configuration. The package install, managed settings (default paths,
# fonts, macro security, WebDAV integration), and extension installation
# are handled by the home-manager module (home-modules/libreoffice.nix).
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.libreoffice;
in
{
  options.services.hearth.libreoffice = {
    enable = lib.mkEnableOption "Hearth managed LibreOffice configuration";

    nextcloudUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud server for WebDAV integration, default save paths, and extension API calls.";
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

    enableExtensions = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Install Hearth LibreOffice extensions (Nextcloud Share, Comments
        sidebar, Lock Status). Requires LibreOffice built with
        --enable-rust-uno (Phase 2).
      '';
    };
  };

  # No system-level config needed — LibreOffice is installed and configured
  # per-user by home-modules/libreoffice.nix via home-manager.
}
