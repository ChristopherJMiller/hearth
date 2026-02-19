# modules/roles/default.nix — Role composition module
#
# Maps role names to sets of NixOS system-level configuration. This is the
# system-level counterpart to the home-manager role profiles in home-modules/.
# Each role can add system packages, enable services, and tweak system config.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth;
  roleCfg = config.services.hearth.roles;

  # Role definitions: each role maps to a set of NixOS configuration changes
  roleConfigs = {
    # Default role: minimal additions beyond the desktop baseline
    default = {
      environment.systemPackages = with pkgs; [
        libreoffice
      ];
    };

    # Developer role: build tools, containers, editors
    developer = {
      environment.systemPackages = with pkgs; [
        # Editors
        neovim
        vscodium

        # Version control
        git
        git-lfs

        # Build toolchains
        gcc
        gnumake
        cmake
        python3
        nodejs

        # Containers
        podman
        podman-compose
        buildah

        # Debugging
        gdb
        strace
        ltrace
        valgrind
      ];

      # Enable podman for rootless containers
      virtualisation.podman = {
        enable = true;
        dockerCompat = true;
        defaultNetwork.settings.dns_enabled = true;
      };

      # Developer-friendly Nix settings
      nix.settings = {
        experimental-features = [ "nix-command" "flakes" ];
        keep-outputs = true;
        keep-derivations = true;
      };
    };

    # Designer role: creative tools and color management
    designer = {
      environment.systemPackages = with pkgs; [
        gimp
        inkscape
        krita
        fontforge

        # Color management
        colord
        argyllcms

        # Image viewers
        loupe
      ];

      # Extra fonts for design work
      fonts.packages = with pkgs; [
        google-fonts
        nerd-fonts.fira-code
        nerd-fonts.jetbrains-mono
        inter
        roboto
        lato
      ];

      # Enable colord for color management
      services.colord.enable = true;
    };

    # Admin/IT role: system administration and network tools
    admin = {
      environment.systemPackages = with pkgs; [
        # System monitoring
        htop
        iotop
        sysstat

        # Network diagnostics
        nmap
        wireshark
        tcpdump
        traceroute
        mtr
        iperf3
        dig
        whois

        # Remote management
        tmux
        screen
        ansible
        openssh

        # Disk utilities
        smartmontools
        hdparm
        nvme-cli

        # Log analysis
        lnav

        # Container management
        podman
        podman-compose

        # Nix tools
        nix-tree
        nix-diff
      ];

      # Admin users can use podman
      virtualisation.podman = {
        enable = true;
        dockerCompat = true;
      };

      # Enable Nix flakes for admin operations
      nix.settings.experimental-features = [ "nix-command" "flakes" ];

      # Enable Wireshark capture permissions
      programs.wireshark.enable = true;
    };
  };
in
{
  options.services.hearth.roles = {
    role = lib.mkOption {
      type = lib.types.enum [ "default" "developer" "designer" "admin" ];
      default = "default";
      description = ''
        The system-level role for this machine. Determines which additional
        packages and services are configured at the NixOS level.
        User-level configuration is handled separately by home-manager
        role profiles.
      '';
    };

    extraRoleConfig = lib.mkOption {
      type = lib.types.attrsOf lib.types.raw;
      default = { };
      description = ''
        Additional NixOS configuration to merge into the selected role.
        Use this for site-specific role customizations.
      '';
    };
  };

  config = lib.mkMerge [
    # Apply the selected role's configuration.
    # Each role is guarded by a separate mkIf to keep evaluation lazy
    # and avoid infinite recursion through the module fixed-point.
    (lib.mkIf (roleCfg.role == "default") roleConfigs.default)
    (lib.mkIf (roleCfg.role == "developer") roleConfigs.developer)
    (lib.mkIf (roleCfg.role == "designer") roleConfigs.designer)
    (lib.mkIf (roleCfg.role == "admin") roleConfigs.admin)
  ];
}
