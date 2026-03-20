# home-modules/chat.nix — Corporate chat (Element Desktop) home-manager module
#
# Pre-configures Element Desktop with the Hearth Synapse homeserver,
# Kanidm SSO (immediate redirect), GNOME Keyring session persistence,
# and optional auto-start on login.
#
# Enabled via hearth.chat.enable when the fleet has chat capability.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.chat;
in
{
  options.hearth.chat = {
    enable = lib.mkEnableOption "Hearth corporate chat (Element Desktop)";

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
      default = true;
      description = "Whether to auto-start Element Desktop on login (minimized to tray).";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- Element Desktop package ---
    home.packages = [ pkgs.element-desktop ];

    # --- Pre-configure Element Desktop ---
    # Points at the Hearth Synapse instance with SSO immediate redirect.
    # disable_custom_urls prevents users from switching to another server.
    xdg.configFile."Element/config.json".text = builtins.toJSON {
      default_server_config = {
        "m.homeserver" = {
          base_url = cfg.homeserverUrl;
          server_name = cfg.serverName;
        };
      };
      brand = "Hearth Chat";
      disable_guests = true;
      disable_3pid_login = true;
      disable_custom_urls = true;
      sso_redirect_options = {
        immediate = true;
      };
      show_labs_settings = false;
      default_theme = "dark";
      room_directory = {
        servers = [ cfg.serverName ];
      };
      setting_defaults = {
        breadcrumbs = true;
        "UIFeature.feedback" = false;
        "UIFeature.registration" = false;
        "UIFeature.thirdPartyId" = false;
        "UIFeature.identityServer" = false;
      };
    };

    # --- Auto-start on GNOME login ---
    # --use-keychain: stores session token in GNOME Keyring for auto-login
    # --hidden: starts minimized to system tray (not intrusive)
    xdg.configFile."autostart/element-desktop.desktop" = lib.mkIf cfg.autoStart {
      text = ''
        [Desktop Entry]
        Type=Application
        Name=Hearth Chat
        Comment=Corporate chat (Element Desktop)
        Exec=element-desktop --use-keychain --hidden
        Icon=element-desktop
        Terminal=false
        X-GNOME-Autostart-enabled=true
        X-GNOME-Autostart-Delay=5
      '';
    };
  };
}
