# Hardware profile for Dell Latitude series (generic).
#
# Provides a baseline configuration for Dell Latitude laptops
# with Intel graphics, power management, and firmware updates.
{ config, lib, pkgs, ... }:

{
  # Intel CPU microcode updates
  hardware.cpu.intel.updateMicrocode = true;

  # Firmware update daemon
  services.fwupd.enable = true;

  # TLP for battery life optimization
  services.tlp.enable = true;

  # Use modesetting driver for display
  services.xserver.videoDrivers = [ "modesetting" ];
}
