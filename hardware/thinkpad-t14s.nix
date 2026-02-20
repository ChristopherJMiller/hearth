# Hardware profile for Lenovo ThinkPad T14s (AMD variant).
#
# Enables AMD-specific optimizations, power management via TLP,
# firmware updates, and Bluetooth support.
{ config, lib, pkgs, ... }:

{
  imports = [
    # TODO: Add nixos-hardware input and import the ThinkPad T14s AMD module:
    # inputs.nixos-hardware.nixosModules.lenovo-thinkpad-t14s-amd-gen1
  ];

  # AMD CPU microcode updates
  hardware.cpu.amd.updateMicrocode = true;

  # Kernel parameters for AMD power management
  boot.kernelParams = [ "amd_pstate=active" ];

  # Firmware update daemon
  services.fwupd.enable = true;

  # TLP for battery life optimization
  services.tlp.enable = true;

  # Bluetooth
  hardware.bluetooth.enable = true;
}
