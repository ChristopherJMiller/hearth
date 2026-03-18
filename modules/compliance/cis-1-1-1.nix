# CIS 1.1.1 — Disable mounting of uncommon filesystems
#
# Prevents loading kernel modules for rarely-used filesystems that could
# be exploited to mount malicious media.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance."cis-1-1-1";
in
{
  options.services.hearth.compliance."cis-1-1-1" = {
    enable = lib.mkEnableOption "CIS 1.1.1 — Disable uncommon filesystem mounting";

    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "CIS-1.1.1";
        title = "Disable mounting of uncommon filesystems";
        severity = "medium";
        description = "Disables cramfs, freevxfs, jffs2, hfs, hfsplus, and squashfs kernel modules to reduce attack surface.";
        family = "filesystem";
        benchmark = "CIS NixOS Level 1";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    boot.blacklistedKernelModules = [
      "cramfs"
      "freevxfs"
      "jffs2"
      "hfs"
      "hfsplus"
    ];
  };
}
