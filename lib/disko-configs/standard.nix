# Standard GPT + EFI + ext4 partitioning scheme for Hearth fleet devices.
#
# Usage in a fleet host configuration:
#   disko.devices = (import ./standard.nix { device = "/dev/nvme0n1"; }).disko.devices;
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
        root = {
          size = "100%";
          content = {
            type = "filesystem";
            format = "ext4";
            mountpoint = "/";
          };
        };
      };
    };
  };
}
