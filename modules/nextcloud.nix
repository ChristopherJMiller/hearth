# modules/nextcloud.nix — NixOS module for Hearth cloud storage on fleet devices
#
# Provides the option interface for mk-fleet-host.nix to wire Nextcloud
# configuration. The package install and per-user configuration
# (server URL, sync client, WebDAV bookmarks) are handled by the
# home-manager nextcloud module (home-modules/nextcloud.nix).
#
# This module enables system-level requirements: GVFS for Nautilus WebDAV
# integration and davfs2 for optional kernel-level WebDAV mounts.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.nextcloud;
in
{
  options.services.hearth.nextcloud = {
    enable = lib.mkEnableOption "Hearth cloud storage support (Nextcloud)";

    serverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://cloud.hearth.example.com";
      description = "URL of the Nextcloud instance on the control plane.";
    };
  };

  config = lib.mkIf cfg.enable {
    # GVFS provides Nautilus with WebDAV (davs://) browsing support
    services.gvfs.enable = true;

    # davfs2 allows kernel-level WebDAV mounts (mount.davfs)
    environment.systemPackages = with pkgs; [
      davfs2
    ];
  };
}
