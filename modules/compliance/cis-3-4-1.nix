# CIS 3.4.1 — Ensure firewall is installed and enabled
#
# A host-based firewall is required to protect the system from unauthorized
# network access.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance."cis-3-4-1";
in
{
  options.services.hearth.compliance."cis-3-4-1" = {
    enable = lib.mkEnableOption "CIS 3.4.1 — Ensure firewall is enabled";

    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "CIS-3.4.1";
        title = "Ensure firewall is installed and enabled";
        severity = "high";
        description = "The NixOS firewall must be enabled to filter inbound traffic and protect the system from unauthorized access.";
        family = "network";
        benchmark = "CIS NixOS Level 1";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    networking.firewall = {
      enable = true;
      logRefusedConnections = true;
    };
  };
}
