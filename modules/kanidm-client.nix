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
        # `name` keeps `@` out of /etc/passwd, the home dir, and `whoami`,
        # which Nextcloud Desktop and other clients mishandle. Must stay
        # in lockstep with flake.nix lib.buildUserEnv home.homeDirectory.
        #
        # home_alias MUST be set to "none" explicitly: in Kanidm 1.10 the
        # alias overrides home_attr in token_homedirectory() (alias result
        # checked first; attr used only when alias is None). And critically,
        # `DEFAULT_HOME_ALIAS = Some(HomeAttr::Spn)` — *omitting* the field
        # falls back to SPN, re-introducing /home/<user>@<domain>. The
        # parser maps the string "none" to Some(None), disabling the alias.
        # Login by either short name or SPN already works without an alias
        # because Kanidm accepts both forms at the auth layer.
        home_attr = "name";
        home_alias = "none";
        use_etc_skel = true;
        hsm_type = cfg.hsmType;
      };
    };

    # Ensure kanidm-unixd restarts on failure (e.g., if the Kanidm server
    # isn't reachable during early boot). The upstream NixOS module doesn't
    # set restart policy, so the daemon dies permanently on first failure.
    systemd.services.kanidm-unixd = {
      after = [ "network-online.target" "nss-lookup.target" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        Restart = "on-failure";
        RestartSec = 5;
        # Give the network time to come up on first boot.
        StartLimitIntervalSec = 120;
        StartLimitBurst = 10;
      };
    };

    systemd.services.kanidm-unixd-tasks = {
      serviceConfig = {
        Restart = "on-failure";
        RestartSec = 5;
        StartLimitIntervalSec = 120;
        StartLimitBurst = 10;
      };
    };
  };
}
