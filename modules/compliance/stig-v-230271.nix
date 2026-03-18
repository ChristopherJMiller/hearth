# STIG V-230271 — Disable USB mass storage
#
# Maps to RHEL STIG V-230271. Prevents USB mass storage devices from being
# used to exfiltrate data or introduce malware.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance."stig-v-230271";
in
{
  options.services.hearth.compliance."stig-v-230271" = {
    enable = lib.mkEnableOption "STIG V-230271 — Disable USB mass storage";

    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "STIG-V-230271";
        title = "System must disable USB mass storage";
        severity = "medium";
        description = "Blacklists the usb-storage kernel module to prevent USB mass storage devices from being mounted.";
        family = "removable-media";
        benchmark = "DISA STIG";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    boot.blacklistedKernelModules = [ "usb-storage" ];
  };
}
