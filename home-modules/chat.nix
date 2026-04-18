# home-modules/chat.nix — Corporate chat (Fractal) home-manager module
#
# Pre-configures Fractal (native GTK4 Matrix client) with the Hearth Synapse
# homeserver and optional auto-start on login. Fractal supports SSO/OAuth 2.0
# login via Kanidm, E2EE, reactions, threads, spaces, and Jitsi VoIP.
#
# Fractal is used instead of Element Desktop for its native GTK4 integration,
# dramatically smaller footprint (~30MB vs ~300MB), and no Electron dependency.
#
# Enabled via hearth.chat.enable when the fleet has chat capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.chat;
in
{
  options.hearth.chat = {
    enable = lib.mkEnableOption "Hearth corporate chat (Fractal)";

    homeserverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://chat.hearth.example.com";
      description = "URL of the Matrix homeserver (Synapse).";
    };

    serverName = lib.mkOption {
      type = lib.types.str;
      example = "hearth.example.com";
      description = "Matrix server name (the domain part of @user:server MXIDs).";
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to auto-start Fractal on login.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ pkgs.fractal ];

    # Fractal uses GNOME Online Accounts or direct login — no config.json needed.
    # On first launch it shows a login screen where the user enters the homeserver
    # URL. With Kanidm OIDC configured on Synapse, it redirects to the SSO flow.
    #
    # To pre-configure the homeserver, we use GNOME's .well-known style:
    # Fractal reads the Matrix well-known from the server, so as long as Synapse
    # serves /.well-known/matrix/client correctly, no client-side config is needed.

    # Auto-start on GNOME login (optional)
    xdg.configFile."autostart/fractal.desktop" = lib.mkIf cfg.autoStart {
      text = ''
        [Desktop Entry]
        Type=Application
        Name=Hearth Chat
        Comment=Corporate chat (Fractal)
        Exec=fractal
        Icon=org.gnome.Fractal
        Terminal=false
        X-GNOME-Autostart-enabled=true
        X-GNOME-Autostart-Delay=5
      '';
    };
  };
}
