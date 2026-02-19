# dev/fleet-vm.nix — Pre-built fleet host VM for rapid development
#
# A standard NixOS VM (using the `virtualisation` module) with GNOME desktop,
# hearth-agent, and hearth-greeter pre-configured. Skips enrollment — this is
# a shortcut for iterating on post-enrollment features (agent polling, greeter
# UI, deployment pipeline).
#
# For the full enrollment→provisioning→install flow, use `just enroll` instead.
#
# Usage:
#   nix run .#fleet-vm
#
# The VM boots with GNOME, auto-logs in as 'dev', and the agent talks to the
# host API server at 10.0.2.2:3000 (QEMU user-mode networking gateway).

{ self, nixpkgs, system ? "x86_64-linux" }:

let
  lib = nixpkgs.lib;
in
nixpkgs.lib.nixosSystem {
  inherit system;

  modules = [
    # Hearth overlay
    {
      nixpkgs.overlays = [
        self.overlays.default
      ];
    }

    # Hearth modules
    ../modules/agent.nix
    ../modules/greeter.nix
    ../modules/pam.nix
    ../modules/desktop.nix
    ../modules/roles/default.nix

    # VM configuration
    ({ config, pkgs, lib, ... }: {
      # --- QEMU VM settings ---
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        graphics = true;
        qemu.options = [
          "-device" "virtio-vga-gl"
          "-display" "gtk,gl=on"
        ];
      };

      # --- System identity ---
      networking = {
        hostName = "hearth-fleet-vm";
        useDHCP = true;
        firewall.enable = false;
      };

      # --- Hearth agent (pre-enrolled) ---
      services.hearth.agent = {
        enable = true;
        serverUrl = "http://10.0.2.2:3000";
        machineId = "00000000-0000-0000-0000-000000000001";
        binaryCacheUrl = "http://10.0.2.2:8080/hearth";
        pollInterval = 10;
        logLevel = "debug";
      };

      # --- Hearth greeter ---
      services.hearth.greeter.enable = true;

      # --- PAM/NSS ---
      services.hearth.pam.enable = true;

      # --- Desktop (GNOME) ---
      services.hearth.desktop.enable = true;

      # --- Role ---
      services.hearth.roles.role = "developer";

      # --- Dev user with auto-login ---
      users.users.dev = {
        isNormalUser = true;
        password = "dev";
        extraGroups = [ "wheel" "hearth" "networkmanager" ];
      };

      security.sudo.wheelNeedsPassword = false;
      services.displayManager.autoLogin = {
        enable = true;
        user = "dev";
      };

      # --- Development utilities ---
      environment.systemPackages = with pkgs; [
        curl
        jq
        htop
        vim
        tmux
        tcpdump
        strace
      ];

      # --- MOTD ---
      environment.etc."motd" = {
        text = ''

          === Hearth Fleet Dev VM ===

          Agent status:   systemctl status hearth-agent
          Agent logs:     journalctl -fu hearth-agent
          Agent config:   cat /etc/hearth/agent.toml
          Agent socket:   /run/hearth/agent.sock

          Host API:       http://10.0.2.2:3000
          Binary cache:   http://10.0.2.2:8080/hearth

        '';
      };

      # --- Nix ---
      nix.settings.experimental-features = [ "nix-command" "flakes" ];

      # --- Boot ---
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = true;

      system.stateVersion = "25.05";
    })
  ];
}
