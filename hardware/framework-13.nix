# Hardware profile for Framework Laptop 13 (Intel variant).
#
# Enables Intel-specific optimizations, firmware updates,
# fingerprint reader support, and display stability fixes.
{ config, lib, pkgs, ... }:

{
  imports = [
    # TODO: Add nixos-hardware input and import the Framework 13 module:
    # inputs.nixos-hardware.nixosModules.framework-13-intel
  ];

  # Intel CPU microcode updates
  hardware.cpu.intel.updateMicrocode = true;

  # Kernel parameters for display stability
  boot.kernelParams = [ "i915.enable_psr=0" ];

  # Firmware update daemon
  services.fwupd.enable = true;

  # Fingerprint reader
  services.fprintd.enable = true;
}
