# modules/chat.nix — NixOS module for Hearth corporate chat on fleet devices
#
# Provides the option interface for mk-fleet-host.nix to wire chat
# configuration. The package install and per-user configuration
# (homeserver URL, SSO, autostart) are handled by the home-manager
# chat module (home-modules/chat.nix).
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.chat;
in
{
  options.services.hearth.chat = {
    enable = lib.mkEnableOption "Hearth corporate chat support";

    homeserverUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://chat.hearth.example.com";
      description = "URL of the Matrix homeserver (Synapse) on the control plane.";
    };

    serverName = lib.mkOption {
      type = lib.types.str;
      example = "hearth.example.com";
      description = "Matrix server name (domain part of @user:server MXIDs).";
    };
  };

  # No system-level config needed — Element Desktop is installed and configured
  # per-user by home-modules/chat.nix via home-manager.
}
