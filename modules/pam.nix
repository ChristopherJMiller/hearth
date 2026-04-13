# modules/pam.nix — NixOS module for PAM/NSS configuration
#
# Wires PAM services for greetd and login to work with the selected
# authentication backend (Kanidm or SSSD). Handles automatic home
# directory creation as a safety net alongside the hearth-agent.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.pam;
  useKanidm = cfg.authBackend == "kanidm";
  useSssd = cfg.authBackend == "sssd";
in
{
  options.services.hearth.pam = {
    enable = lib.mkEnableOption "Hearth PAM/NSS integration";

    authBackend = lib.mkOption {
      type = lib.types.enum [ "kanidm" "sssd" "none" ];
      default = "kanidm";
      description = ''
        Which authentication backend to use for PAM/NSS.
        "kanidm" — uses kanidm-unixd (configured via services.hearth.kanidmClient)
        "sssd" — uses SSSD (site-specific domain config required)
        "none" — local users only (useful for testing)
      '';
    };

    enableMkhomedir = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to enable pam_mkhomedir for automatic home directory creation.
        When using Kanidm, this is force-disabled because kanidm-unixd-tasks
        manages home directories using UUID-based paths with SPN symlinks.
        pam_mkhomedir would create a real directory at the SPN path,
        preventing the symlink from being created.
      '';
    };

    sssdServices = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "nss" "pam" "ssh" ];
      description = "SSSD services to enable (only used when authBackend = sssd).";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      # --- PAM service configuration (always applied) ---
      security.pam.services = {
        greetd.makeHomeDir = cfg.enableMkhomedir;
        login.makeHomeDir = cfg.enableMkhomedir;
        sshd.makeHomeDir = cfg.enableMkhomedir;
      };
    }

    # kanidm-unixd-tasks manages home dirs; pam_mkhomedir would conflict.
    (lib.mkIf useKanidm {
      services.hearth.pam.enableMkhomedir = lib.mkDefault false;
    })

    # --- SSSD integration (legacy backend) ---
    (lib.mkIf useSssd {
      services.sssd = {
        enable = true;
        settings = {
          sssd.services = lib.concatStringsSep ", " cfg.sssdServices;
          nss = {};
          pam = {};
        };
      };

      system.nssDatabases = {
        passwd = lib.mkAfter [ "sss" ];
        group = lib.mkAfter [ "sss" ];
        shadow = lib.mkAfter [ "sss" ];
      };

      security.pam.services = {
        greetd = { sssdStrictAccess = false; pamMount = true; };
        login.sssdStrictAccess = false;
        sshd.sssdStrictAccess = false;
      };
    })

    # --- Home directory skeleton ---
    (lib.mkIf cfg.enableMkhomedir {
      environment.etc."skel/.bashrc" = {
        text = ''
          # Hearth managed workstation — default .bashrc
          # This file is created on first login. Home-manager activation
          # will replace it with the role-appropriate configuration.

          # If not running interactively, don't do anything
          case $- in
              *i*) ;;
                *) return;;
          esac

          # Basic prompt
          PS1='\u@\h:\w\$ '

          # Default aliases
          alias ls='ls --color=auto'
          alias ll='ls -la'
        '';
        mode = "0644";
      };
    })
  ]);
}
