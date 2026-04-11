# modules/greeter.nix — NixOS module for hearth-greeter with greetd
#
# Configures greetd as the display manager and hearth-greeter as the greeter
# binary. The greeter handles authentication, communicates with hearth-agent
# for user environment preparation, and launches the desktop session.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.greeter;
  settingsFormat = pkgs.formats.toml { };

  greeterConfig = settingsFormat.generate "greeter.toml" {
    branding = {
      organization_name = cfg.branding.organizationName;
      logo_path =
        if cfg.branding.logo != null
        then toString cfg.branding.logo
        else "${cfg.package}/share/hearth/default-logo.svg";
      css_path =
        if cfg.branding.cssOverride != null
        then toString cfg.branding.cssOverride
        else "${cfg.package}/share/hearth/hearth-greeter.css";
    };
    agent = {
      socket_path = config.services.hearth.agent.socketPath;
      timeout_secs = cfg.agentTimeout;
    };
    session = {
      command = cfg.sessionCommand;
      fallback_command = cfg.fallbackSessionCommand;
    };
  };
in
{
  options.services.hearth.greeter = {
    enable = lib.mkEnableOption "Hearth login greeter (greetd-based)";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.hearth-greeter;
      defaultText = lib.literalExpression "pkgs.hearth-greeter";
      description = "The hearth-greeter package to use.";
    };

    branding = {
      organizationName = lib.mkOption {
        type = lib.types.str;
        default = "Your Organization";
        description = "Organization name displayed on the login screen.";
      };

      logo = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = "Path to an SVG or PNG logo for the login screen. Uses default Hearth logo if null.";
      };

      cssOverride = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = "Path to a custom GTK4 CSS file for greeter theming.";
      };
    };

    sessionCommand = lib.mkOption {
      type = lib.types.str;
      default = "gnome-session";
      description = "Desktop session command to launch after successful login and environment preparation.";
    };

    fallbackSessionCommand = lib.mkOption {
      type = lib.types.str;
      default = "gnome-session";
      description = "Fallback session command if agent-based preparation fails.";
    };

    agentTimeout = lib.mkOption {
      type = lib.types.int;
      default = 120;
      description = "Timeout in seconds for agent environment preparation before offering fallback.";
    };
  };

  config = lib.mkIf cfg.enable {
    # The greeter requires the agent to be enabled for environment preparation
    assertions = [
      {
        assertion = config.services.hearth.agent.enable;
        message = "services.hearth.greeter requires services.hearth.agent to be enabled.";
      }
    ];

    # Configure greetd as the display manager.
    # The agent IPC socket is managed by systemd socket activation
    # (hearth-agent.socket) with SocketGroup=hearth, so the greeter can
    # access it as long as the greeter user is in the hearth group.
    # greetd doesn't call initgroups(), but systemd socket permissions
    # are set at socket creation time, not checked via supplementary groups.
    services.greetd = {
      enable = true;
      settings = {
        default_session = {
          # Wrap the greeter in cage (kiosk Wayland compositor) so GTK4
          # has a display to render on. cage exits when the greeter exits,
          # then greetd starts the user's desktop session.
          command = "${pkgs.cage}/bin/cage -s -- ${cfg.package}/bin/hearth-greeter";
          user = "greeter";
        };
        # Terminal VT to use for the greeter
        terminal.vt = 1;
      };
    };

    # seatd provides seat/DRM access for the cage compositor.
    services.seatd.enable = true;

    # Ensure greetd starts after the agent socket and seatd are available
    systemd.services.greetd.after = [ "hearth-agent.socket" "seatd.service" ];

    # Allow cage to run without input devices (common in VMs).
    systemd.services.greetd.environment.WLR_LIBINPUT_NO_DEVICES = "1";

    # Disable GDM since we use greetd
    services.displayManager.gdm.enable = lib.mkForce false;

    # Generate greeter configuration
    environment.etc."hearth/greeter.toml" = {
      source = greeterConfig;
      mode = "0644";
    };

    # The greeter user needs access to the agent socket
    users.users.greeter = {
      isSystemUser = true;
      group = "greeter";
      extraGroups = [ "hearth" "video" "render" ];
      description = "greetd greeter user";
    };
    users.groups.greeter = { };

    # Ensure greeter can access GPU for Wayland rendering
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (action.id == "org.freedesktop.login1.set-session" &&
            subject.user == "greeter") {
          return polkit.Result.YES;
        }
      });
    '';
  };
}
