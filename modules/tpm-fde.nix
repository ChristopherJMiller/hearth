# modules/tpm-fde.nix — TPM2-backed Full Disk Encryption for Hearth fleet devices
#
# Configures LUKS disk encryption with TPM2 auto-unlock. On first boot, a oneshot
# service enrolls the TPM2 device against the specified PCR registers so that
# subsequent boots can unseal the LUKS key automatically without user interaction.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.tpmFde;
  pcrString = lib.concatMapStringsSep "+" toString cfg.tpmPcrList;
in
{
  options.services.hearth.tpmFde = {
    enable = lib.mkEnableOption "TPM2-backed Full Disk Encryption for Hearth fleet devices";

    device = lib.mkOption {
      type = lib.types.str;
      default = "/dev/disk/by-partlabel/cryptroot";
      description = "Path to the LUKS-encrypted block device.";
    };

    tpmPcrList = lib.mkOption {
      type = lib.types.listOf lib.types.int;
      default = [ 0 2 7 ];
      description = ''
        List of TPM2 PCR registers to bind the LUKS key to.
        Common choices:
        - PCR 0: firmware (BIOS/UEFI)
        - PCR 2: option ROMs
        - PCR 7: Secure Boot state
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # Point the initrd LUKS configuration at the target device
    boot.initrd.luks.devices.cryptroot.device = cfg.device;

    # systemd-based initrd is required for systemd-cryptenroll TPM2 unlock
    boot.initrd.systemd.enable = true;

    # Ensure systemd (for systemd-cryptenroll) is available on the system
    environment.systemPackages = [ pkgs.systemd ];

    # First-boot oneshot: enroll the TPM2 device for LUKS auto-unlock
    systemd.services.hearth-tpm-enroll = {
      description = "Hearth TPM2 LUKS Enrollment";
      documentation = [ "https://www.freedesktop.org/software/systemd/man/systemd-cryptenroll.html" ];

      after = [ "local-fs.target" ];
      wantedBy = [ "multi-user.target" ];

      unitConfig = {
        ConditionPathExists = "!/var/lib/hearth/.tpm-enrolled";
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.systemd}/bin/systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=${pcrString} ${cfg.device}";
        ExecStartPost = "${pkgs.coreutils}/bin/touch /var/lib/hearth/.tpm-enrolled";
        StateDirectory = "hearth";
      };
    };
  };
}
