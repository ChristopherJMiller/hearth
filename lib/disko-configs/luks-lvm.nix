# LUKS-encrypted LVM partitioning with separate /home for Hearth fleet devices.
#
# Provides full-disk encryption with logical volume management.
# The LUKS passphrase is prompted at boot.
#
# Usage in a fleet host configuration:
#   disko.devices = (import ./luks-lvm.nix { device = "/dev/nvme0n1"; }).disko.devices;
{ device }:
{
  disko.devices.disk.main = {
    inherit device;
    type = "disk";
    content = {
      type = "gpt";
      partitions = {
        ESP = {
          size = "512M";
          type = "EF00";
          content = {
            type = "filesystem";
            format = "vfat";
            mountpoint = "/boot";
          };
        };
        luks = {
          size = "100%";
          content = {
            type = "luks";
            name = "cryptroot";
            settings = {
              allowDiscards = true;
            };
            content = {
              type = "lvm_pv";
              vg = "vg";
            };
          };
        };
      };
    };
  };

  disko.devices.lvm_vg.vg = {
    type = "lvm_vg";
    lvs = {
      root = {
        size = "50G";
        content = {
          type = "filesystem";
          format = "ext4";
          mountpoint = "/";
        };
      };
      home = {
        size = "100%FREE";
        content = {
          type = "filesystem";
          format = "ext4";
          mountpoint = "/home";
        };
      };
    };
  };
}
