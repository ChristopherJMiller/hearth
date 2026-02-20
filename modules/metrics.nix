# modules/metrics.nix — Fleet metrics collection with VictoriaMetrics Agent (vmagent)
#
# Deploys vmagent on each fleet device to scrape local metrics from
# node_exporter and Hearth agent textfile metrics, then remote-write them
# to a central VictoriaMetrics or Prometheus-compatible endpoint.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.metrics;
  textfileDir = "/var/lib/prometheus-node-exporter";
in
{
  options.services.hearth.metrics = {
    enable = lib.mkEnableOption "fleet metrics collection via vmagent for Hearth fleet devices";

    remoteWriteUrl = lib.mkOption {
      type = lib.types.str;
      example = "http://victoriametrics.example.com:8428/api/v1/write";
      description = "Prometheus remote-write endpoint URL.";
    };

    scrapeInterval = lib.mkOption {
      type = lib.types.str;
      default = "30s";
      description = "How often vmagent scrapes local metric targets.";
    };
  };

  config = lib.mkIf cfg.enable {
    # VictoriaMetrics Agent for remote-write
    services.vmagent = {
      enable = true;

      remoteWrite.url = cfg.remoteWriteUrl;

      prometheusConfig = {
        global = {
          scrape_interval = cfg.scrapeInterval;
        };

        scrape_configs = [
          {
            job_name = "node";
            static_configs = [
              { targets = [ "localhost:9100" ]; }
            ];
          }
          {
            job_name = "hearth-agent";
            static_configs = [
              { targets = [ "localhost:9100" ]; }
            ];
            metric_relabel_configs = [
              {
                source_labels = [ "__name__" ];
                regex = "node_textfile_.*";
                action = "keep";
              }
            ];
          }
        ];
      };
    };

    # Prometheus node_exporter with textfile collector for Hearth agent metrics
    services.prometheus.exporters.node = {
      enable = true;
      enabledCollectors = [ "textfile" ];
      extraFlags = [
        "--collector.textfile.directory=${textfileDir}"
      ];
    };

    # Ensure the textfile collector directory exists
    systemd.tmpfiles.rules = [
      "d ${textfileDir} 0755 root root -"
    ];
  };
}
