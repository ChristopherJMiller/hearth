# home-modules/services.nix — Service directory desktop integration
#
# Surfaces platform services (chat, cloud, identity, etc.) as desktop
# bookmarks. The agent writes service metadata to /var/lib/hearth/services/
# from the heartbeat response; this module exposes those as .desktop links
# in the user's application menu.
#
# Enabled via hearth.services.enable when the fleet has collaboration services.
{ config, lib, pkgs, ... }:

let
  cfg = config.hearth.services;
in
{
  options.hearth.services = {
    enable = lib.mkEnableOption "Hearth service directory desktop integration";

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = /var/lib/hearth/services;
      description = "Directory where the agent writes service metadata.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Symlink agent-provided .desktop files into the user's applications dir
    # so they appear in the GNOME application grid / search.
    systemd.user.services.hearth-service-bookmarks = {
      Unit = {
        Description = "Hearth service bookmarks sync";
        After = [ "graphical-session-pre.target" ];
      };

      Service = {
        Type = "oneshot";
        ExecStart = let
          script = pkgs.writeShellScript "hearth-sync-services" ''
            src="${toString cfg.dataDir}"
            dest="$HOME/.local/share/applications"
            mkdir -p "$dest"
            if [ -d "$src" ]; then
              for f in "$src"/hearth-*.desktop; do
                [ -f "$f" ] && cp "$f" "$dest/" 2>/dev/null || true
              done
            fi
          '';
        in "${script}";
      };

      Install.WantedBy = [ "graphical-session.target" ];
    };

    # Run the sync periodically so new services get picked up
    systemd.user.timers.hearth-service-bookmarks = {
      Unit.Description = "Periodic Hearth service bookmarks sync";
      Timer = {
        OnActiveSec = "1min";
        OnUnitActiveSec = "30min";
      };
      Install.WantedBy = [ "timers.target" ];
    };
  };
}
