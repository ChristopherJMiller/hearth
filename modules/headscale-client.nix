# modules/headscale-client.nix — NixOS module for Headscale mesh VPN
#
# Configures Tailscale on fleet devices to connect to a Headscale coordination
# server. On first boot, a oneshot service uses a pre-auth key to join the mesh
# automatically, then deletes the key file.
#
# IT admins can SSH into any enrolled device via its Headscale mesh IP.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.headscaleClient;
in
{
  options.services.hearth.headscaleClient = {
    enable = lib.mkEnableOption "Hearth Headscale mesh VPN client";

    serverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://headscale.hearth.example.com";
      description = "URL of the Headscale coordination server.";
    };

    preauthKeyFile = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/hearth/headscale-key";
      description = ''
        Path to the pre-auth key file. Written during enrollment provisioning.
        The key is consumed (deleted) after a successful mesh join.
      '';
    };

    acceptRoutes = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to accept advertised routes from the mesh.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Enable the Tailscale daemon.
    services.tailscale.enable = true;

    # Oneshot service to join the Headscale mesh on first boot.
    # Only runs if the pre-auth key file exists (i.e., freshly provisioned).
    systemd.services.hearth-headscale-join = {
      description = "Join Headscale mesh VPN";
      after = [ "tailscaled.service" "network-online.target" ];
      wants = [ "tailscaled.service" "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      unitConfig = {
        ConditionPathExists = cfg.preauthKeyFile;
      };

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = pkgs.writeShellScript "hearth-headscale-join" ''
          # Wait for tailscaled to be ready
          for i in $(seq 1 10); do
            ${pkgs.tailscale}/bin/tailscale status >/dev/null 2>&1 && break
            sleep 1
          done

          KEY=$(cat ${cfg.preauthKeyFile})
          ${pkgs.tailscale}/bin/tailscale up \
            --login-server="${cfg.serverUrl}" \
            --authkey="$KEY"${lib.optionalString cfg.acceptRoutes " --accept-routes"}
        '';
        ExecStartPost = "${pkgs.coreutils}/bin/rm -f ${cfg.preauthKeyFile}";
      };
    };

    # Open WireGuard UDP port for Tailscale.
    networking.firewall.allowedUDPPorts = [ 41641 ];

    # Ensure SSH is available for IT remote access over the mesh.
    services.openssh.enable = true;

    # Make tailscale CLI available.
    environment.systemPackages = [ pkgs.tailscale ];
  };
}
