# modules/logging.nix — Centralized logging with Promtail/Loki for Hearth fleet devices
#
# Ships system journal logs from each fleet device to a central Loki instance
# via Promtail. Supports adding extra static labels (e.g. fleet, environment)
# for log filtering and correlation.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.logging;
  hostname = config.networking.hostName;

  # Build the static_configs labels by merging extraLabels with the hostname
  staticLabels = { host = hostname; } // cfg.extraLabels;
in
{
  options.services.hearth.logging = {
    enable = lib.mkEnableOption "centralized logging via Promtail/Loki for Hearth fleet devices";

    lokiUrl = lib.mkOption {
      type = lib.types.str;
      example = "http://loki.example.com:3100/loki/api/v1/push";
      description = "URL of the Loki push API endpoint.";
    };

    extraLabels = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = {
        fleet = "production";
        site = "hq";
      };
      description = ''
        Additional static labels to attach to all log streams.
        The "host" label is always added automatically from the system hostname.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.promtail = {
      enable = true;

      configuration = {
        server = {
          http_listen_port = 28183;
          grpc_listen_port = 0;
        };

        positions = {
          filename = "/var/lib/promtail/positions.yaml";
        };

        clients = [
          { url = cfg.lokiUrl; }
        ];

        scrape_configs = [
          {
            job_name = "journal";
            journal = {
              max_age = "12h";
              labels = staticLabels;
            };
            relabel_configs = [
              {
                source_labels = [ "__journal__hostname" ];
                target_label = "host";
              }
              {
                source_labels = [ "__journal__systemd_unit" ];
                target_label = "unit";
              }
              {
                source_labels = [ "__journal_priority_keyword" ];
                target_label = "level";
              }
            ];
          }
        ];
      };
    };
  };
}
