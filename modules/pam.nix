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
        This acts as a safety net — the hearth-agent creates home directories
        as its primary mechanism, but pam_mkhomedir ensures a bare home
        directory exists even if the agent path fails.
      '';
    };

    sssdServices = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "nss" "pam" "ssh" ];
      description = "SSSD services to enable (only used when authBackend = sssd).";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- SSSD integration (legacy backend) ---
    services.sssd = lib.mkIf useSssd {
      enable = true;
      # Note: the actual sssd.conf with domain configuration is site-specific.
      # Fleet operators provide this via extraConfig or sops-nix secrets.
      # We just ensure the SSSD service is running and PAM is wired.
      sshAuthorizedKeysIntegration = true;
    };

    # --- NSS configuration ---
    system.nssDatabases = lib.mkIf useSssd {
      passwd = lib.mkAfter [ "sss" ];
      group = lib.mkAfter [ "sss" ];
      shadow = lib.mkAfter [ "sss" ];
    };
    # Note: when authBackend = "kanidm", NSS is configured automatically
    # by NixOS's services.kanidm.enablePam option (via kanidm-client.nix).

    # --- PAM service configuration ---
    security.pam.services = {
      greetd = if useSssd then {
        sssdStrictAccess = false;
        makeHomeDir = cfg.enableMkhomedir;
        pamMount = true;
      } else {
        # kanidm: PAM auth configured by services.kanidm.enablePam;
        # we only add mkhomedir as a safety net.
        makeHomeDir = cfg.enableMkhomedir;
      };

      login = if useSssd then {
        sssdStrictAccess = false;
        makeHomeDir = cfg.enableMkhomedir;
      } else {
        makeHomeDir = cfg.enableMkhomedir;
      };

      sshd = if useSssd then {
        sssdStrictAccess = false;
        makeHomeDir = cfg.enableMkhomedir;
      } else {
        makeHomeDir = cfg.enableMkhomedir;
      };
    };

    # --- Home directory skeleton ---
    # Ensure the home directory skeleton has sensible defaults
    environment.etc."skel/.bashrc" = lib.mkIf cfg.enableMkhomedir {
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
  };
}
