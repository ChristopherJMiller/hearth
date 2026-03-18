# STIG V-230223 — SSH must use protocol version 2 with hardened config
#
# Maps to RHEL STIG V-230223. Ensures SSH daemon uses strong cryptographic
# settings and disables insecure options.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance."stig-v-230223";
in
{
  options.services.hearth.compliance."stig-v-230223" = {
    enable = lib.mkEnableOption "STIG V-230223 — SSH hardened configuration";

    meta = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      default = {
        id = "STIG-V-230223";
        title = "SSH must use protocol version 2 with hardened configuration";
        severity = "high";
        description = "Configures OpenSSH daemon with strong ciphers, MACs, key exchange algorithms, disables root login, password authentication, and empty passwords.";
        family = "access-control";
        benchmark = "DISA STIG";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    services.openssh = {
      enable = true;
      settings = {
        PermitRootLogin = "no";
        PasswordAuthentication = false;
        PermitEmptyPasswords = false;
        X11Forwarding = false;
        MaxAuthTries = 4;
        ClientAliveInterval = 600;
        ClientAliveCountMax = 0;
        LoginGraceTime = 60;
      };
      extraConfig = ''
        # STIG V-230223: Strong cryptographic settings
        Ciphers aes256-gcm@openssh.com,aes128-gcm@openssh.com,aes256-ctr,aes192-ctr,aes128-ctr
        MACs hmac-sha2-512-etm@openssh.com,hmac-sha2-256-etm@openssh.com,hmac-sha2-512,hmac-sha2-256
        KexAlgorithms curve25519-sha256,curve25519-sha256@libssh.org,diffie-hellman-group16-sha512,diffie-hellman-group18-sha512
      '';
    };
  };
}
