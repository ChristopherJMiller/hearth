# modules/pam.nix — NixOS module for PAM/NSS configuration
#
# Wires PAM services for greetd and login to work with SSSD-based
# authentication and automatic home directory creation. The actual SSSD
# configuration (domain, IdP connection, etc.) is site-specific and
# not managed here.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.pam;
in
{
  options.services.hearth.pam = {
    enable = lib.mkEnableOption "Hearth PAM/NSS integration";

    enableSssd = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to enable SSSD integration for PAM authentication.
        When enabled, configures PAM services to use SSSD for user
        authentication against an external identity provider.
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
      description = "SSSD services to enable.";
    };
  };

  config = lib.mkIf cfg.enable {
    # --- SSSD integration ---
    services.sssd = lib.mkIf cfg.enableSssd {
      enable = true;
      # Note: the actual sssd.conf with domain configuration is site-specific.
      # Fleet operators provide this via extraConfig or sops-nix secrets.
      # We just ensure the SSSD service is running and PAM is wired.
      sshAuthorizedKeysIntegration = true;
    };

    # --- NSS configuration ---
    # Ensure SSSD is in the NSS lookup chain for passwd, group, shadow
    system.nssDatabases = lib.mkIf cfg.enableSssd {
      passwd = lib.mkAfter [ "sss" ];
      group = lib.mkAfter [ "sss" ];
      shadow = lib.mkAfter [ "sss" ];
    };

    # --- PAM service configuration ---
    # Configure PAM for the greetd greeter session
    security.pam.services.greetd = lib.mkIf cfg.enableSssd {
      # Enable SSSD for authentication
      sssdStrictAccess = false;
      # Home directory creation as a safety net
      makeHomeDir = cfg.enableMkhomedir;
      # Allow PAM mount for network shares if configured
      pamMount = true;
      # Do NOT configure home-manager activation here —
      # that is owned by the greeter-to-agent flow
    };

    # Configure PAM for console login sessions
    security.pam.services.login = lib.mkIf cfg.enableSssd {
      sssdStrictAccess = false;
      makeHomeDir = cfg.enableMkhomedir;
    };

    # Configure PAM for SSH sessions
    security.pam.services.sshd = lib.mkIf cfg.enableSssd {
      sssdStrictAccess = false;
      makeHomeDir = cfg.enableMkhomedir;
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
