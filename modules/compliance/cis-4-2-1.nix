# CIS 4.2.1 — Ensure journald is configured for persistent logging
#
# System logs must be preserved across reboots for security auditing
# and incident response.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance."cis-4-2-1";
in
{
  options.services.hearth.compliance."cis-4-2-1" = {
    enable = lib.mkEnableOption "CIS 4.2.1 — Persistent journald logging";

    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "CIS-4.2.1";
        title = "Ensure journald is configured for persistent logging";
        severity = "medium";
        description = "Configures systemd-journald with persistent storage, compression, and reasonable size limits.";
        family = "logging";
        benchmark = "CIS NixOS Level 1";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    services.journald.extraConfig = ''
      Storage=persistent
      Compress=yes
      SystemMaxUse=2G
      SystemKeepFree=1G
      MaxRetentionSec=90day
    '';
  };
}
