# modules/agent.nix — NixOS module for the hearth-agent systemd service
#
# The agent is the primary on-device daemon. It runs as a systemd system service,
# communicates with the Hearth control plane, manages user environment activation,
# and exposes a Unix socket API for the greeter.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.agent;
  settingsFormat = pkgs.formats.toml { };

  # Build the agent.toml configuration from module options
  agentConfig = settingsFormat.generate "agent.toml" ({
    server.url = cfg.serverUrl;
    server.machine_id = cfg.machineId;
    agent = {
      poll_interval_secs = cfg.pollInterval;
      socket_path = cfg.socketPath;
      machine_token_path = cfg.machineTokenPath;
    };
    cache = lib.optionalAttrs (cfg.binaryCacheUrl != null) {
      url = cfg.binaryCacheUrl;
    };
    home = lib.optionalAttrs (cfg.homeFlakeRef != null) {
      flake_ref = cfg.homeFlakeRef;
    };
  } // lib.optionalAttrs (cfg.roleMapping != []) {
    role_mapping = {
      mappings = cfg.roleMapping;
      default_role = cfg.defaultRole;
    };
  } // lib.optionalAttrs (cfg.headscale.enable) {
    headscale = {
      report_ip = cfg.headscale.reportIp;
    } // lib.optionalAttrs (cfg.headscale.meshServerUrl != null) {
      mesh_server_url = cfg.headscale.meshServerUrl;
    };
  });
in
{
  options.services.hearth.agent = {
    enable = lib.mkEnableOption "Hearth fleet management agent";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.hearth-agent;
      defaultText = lib.literalExpression "pkgs.hearth-agent";
      description = "The hearth-agent package to use.";
    };

    serverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://api.hearth.example.com";
      description = "URL of the Hearth control plane API endpoint.";
    };

    machineId = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
      description = ''
        Machine UUID assigned during enrollment. If empty, the agent will
        attempt to read it from /var/lib/hearth/machine-id on first boot.
      '';
    };

    pollInterval = lib.mkOption {
      type = lib.types.int;
      default = 60;
      description = "Interval in seconds between control plane polling cycles.";
    };

    socketPath = lib.mkOption {
      type = lib.types.str;
      default = "/run/hearth/agent.sock";
      description = "Path to the Unix domain socket for greeter-agent IPC.";
    };

    binaryCacheUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "https://cache.hearth.example.com/fleet-prod";
      description = "Attic binary cache URL for closure pulls. Null to use system defaults.";
    };

    roleMapping = lib.mkOption {
      type = lib.types.listOf (lib.types.submodule {
        options = {
          group = lib.mkOption {
            type = lib.types.str;
            description = "Identity provider group name.";
          };
          role = lib.mkOption {
            type = lib.types.str;
            description = "Hearth role profile name.";
          };
        };
      });
      default = [ ];
      example = [
        { group = "engineering"; role = "developer"; }
        { group = "design"; role = "designer"; }
        { group = "it-admin"; role = "admin"; }
      ];
      description = ''
        Priority-ordered list of identity provider group to Hearth role mappings.
        The first matching group wins. If none match, defaultRole is used.
      '';
    };

    defaultRole = lib.mkOption {
      type = lib.types.str;
      default = "default";
      description = "Fallback role when no role mapping matches the user's groups.";
    };

    logLevel = lib.mkOption {
      type = lib.types.enum [ "trace" "debug" "info" "warn" "error" ];
      default = "info";
      description = "Log level for the hearth-agent process.";
    };

    metricsPath = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/prometheus-node-exporter/hearth.prom";
      description = "Path for Prometheus textfile metrics export.";
    };

    machineTokenPath = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/hearth/machine-token";
      description = "Path to the machine auth token file, written during enrollment.";
    };

    homeFlakeRef = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "github:myorg/fleet-config";
      description = ''
        Flake reference used for user environment activation via
        `home-manager switch --flake <ref>`. Null to disable
        home-manager based user environment management.
      '';
    };

    headscale = {
      enable = lib.mkEnableOption "Headscale mesh VPN IP reporting in heartbeats";

      reportIp = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Report the Headscale mesh IP address in heartbeats.";
      };

      meshServerUrl = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "http://100.64.0.1:3000";
        description = ''
          Optional control plane URL reachable over the Headscale mesh.
          When set, the agent tries this URL first and falls back to
          the public serverUrl on failure.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    # Create the hearth system user and group
    users.users.hearth = {
      isSystemUser = true;
      group = "hearth";
      home = "/var/lib/hearth";
      description = "Hearth fleet agent service user";
    };
    users.groups.hearth = { };

    # Generate the agent configuration file
    environment.etc."hearth/agent.toml" = {
      source = agentConfig;
      mode = "0640";
      user = "root";
      group = "hearth";
    };

    # Ensure the nix CLI is available for store operations
    environment.systemPackages = [ pkgs.nix ];

    # Systemd socket unit for the agent IPC socket. Systemd creates the
    # socket with correct ownership and permissions, solving the problem of
    # greetd not applying supplementary groups to the greeter process.
    # The greeter user can access the socket via the hearth group.
    systemd.sockets.hearth-agent = {
      description = "Hearth Agent IPC Socket";
      wantedBy = [ "sockets.target" ];

      socketConfig = {
        ListenStream = cfg.socketPath;
        # Group is set to "greeter" so the greeter process can connect.
        # greetd doesn't call initgroups() for the greeter user, so
        # supplementary groups aren't available. Using the greeter's
        # primary group ensures access without workarounds.
        SocketGroup = "greeter";
        SocketMode = "0660";
        DirectoryMode = "0755";

        # Remove stale socket on restart
        RemoveOnStop = true;
      };
    };

    # The agent systemd service
    systemd.services.hearth-agent = {
      description = "Hearth Fleet Management Agent";
      documentation = [ "https://github.com/hearth-os/hearth" ];

      after = [ "network-online.target" "nss-lookup.target" ];
      wants = [ "network-online.target" ];
      requires = [ "hearth-agent.socket" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        RUST_LOG = "hearth_agent=${cfg.logLevel}";
        HEARTH_CONFIG = "/etc/hearth/agent.toml";
        HEARTH_METRICS_PATH = cfg.metricsPath;
      };

      path = [ pkgs.nix ]
        ++ lib.optional (cfg.homeFlakeRef != null) pkgs.home-manager
        ++ lib.optional cfg.headscale.enable pkgs.tailscale;

      serviceConfig = {
        Type = "notify";
        ExecStart = "${cfg.package}/bin/hearth-agent";
        Restart = "always";
        RestartSec = 5;
        # WatchdogSec requires periodic WATCHDOG=1 pings from the agent;
        # not yet implemented, so leave it unset to avoid spurious kills.

        # Directories
        RuntimeDirectory = "hearth";
        RuntimeDirectoryMode = "0750";
        StateDirectory = "hearth";
        StateDirectoryMode = "0750";

        # The agent runs switch-to-configuration which executes NixOS
        # activation scripts. These scripts need write access to /etc,
        # /usr/bin, /run/systemd, /proc/sys, and more — essentially full
        # system access. We therefore cannot use ProtectSystem or
        # ProtectKernelTunables here.
        ProtectHome = false; # needs to manage /home
        PrivateTmp = true;
        NoNewPrivileges = false; # must setuid for activation scripts
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = false; # activation scripts may need this
        LockPersonality = true;

        # Logging
        StandardOutput = "journal";
        StandardError = "journal";
        SyslogIdentifier = "hearth-agent";
      };
    };

    # Ensure the metrics directory exists for prometheus node-exporter textfile
    systemd.tmpfiles.rules = [
      "d ${builtins.dirOf cfg.metricsPath} 0755 root root -"
    ];

    # The agent only makes outbound connections — no firewall ports to open
    # But ensure the socket directory has correct permissions for greeter access
    systemd.tmpfiles.settings."10-hearth" = {
      "/run/hearth" = {
        d = {
          user = "root";
          group = "hearth";
          mode = "0750";
        };
      };
    };
  };
}
