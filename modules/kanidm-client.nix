# modules/kanidm-client.nix — NixOS module for Kanidm PAM/NSS integration
#
# Configures kanidm-unixd on fleet devices so that Kanidm users can log in
# via PAM (greetd, login, sshd) and are resolvable via NSS (passwd, group).
# This replaces SSSD as the identity backend for Hearth-managed workstations.
#
# Requires a running Kanidm server accessible at the configured URL.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.kanidmClient;
in
{
  options.services.hearth.kanidmClient = {
    enable = lib.mkEnableOption "Hearth Kanidm client (PAM/NSS via kanidm-unixd)";

    uri = lib.mkOption {
      type = lib.types.str;
      example = "https://idm.hearth.example.com";
      description = "URL of the Kanidm server.";
    };

    caCertPath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to the CA certificate for TLS verification against the Kanidm
        server. If null, the system CA bundle is used.
      '';
    };

    allowedLoginGroups = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "hearth-users" ];
      description = ''
        Kanidm groups whose members are allowed to log in via PAM.
        Members of any listed group can authenticate on this device.
      '';
    };

    defaultShell = lib.mkOption {
      type = lib.types.str;
      default = "/run/current-system/sw/bin/bash";
      description = "Default shell for Kanidm users who don't have one set.";
    };

    homePrefix = lib.mkOption {
      type = lib.types.str;
      default = "/home/";
      description = "Prefix for Kanidm user home directories.";
    };

    hsmType = lib.mkOption {
      type = lib.types.enum [ "soft" "tpm" "tpm_if_possible" ];
      default = "soft";
      description = ''
        HSM type for kanidm-unixd credential storage.
        "soft" uses software encryption (works everywhere).
        "tpm" requires a hardware TPM.
        "tpm_if_possible" uses TPM when available, falls back to soft.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # --- Kanidm client + PAM/NSS daemon ---
    services.kanidm = {
      client.enable = true;
      unix.enable = true;
      # Uses pkgs.kanidm which is pinned to 1.9 via the Hearth overlay.

      client.settings = {
        uri = cfg.uri;
      } // lib.optionalAttrs (cfg.caCertPath != null) {
        ca_path = cfg.caCertPath;
      };

      unix.settings = {
        kanidm.pam_allowed_login_groups = cfg.allowedLoginGroups;
        default_shell = cfg.defaultShell;
        home_prefix = cfg.homePrefix;
        home_attr = "uuid";
        home_alias = "spn";
        use_etc_skel = true;
        hsm_type = cfg.hsmType;
      };
    };
  };
}
