# modules/services.nix — Service directory desktop integration
#
# Marks the machine as having service directory awareness enabled.
# The hearth-agent uses this to manage desktop shortcuts and
# XDG entries for organizational services (chat, cloud, etc.).
{ config, lib, ... }:

let
  cfg = config.services.hearth.services;
in
{
  options.services.hearth.services = {
    enable = lib.mkEnableOption "Hearth service directory desktop integration";
  };

  config = lib.mkIf cfg.enable {
    # The agent handles runtime service discovery and desktop integration.
    # This module just ensures the option is declared so mk-fleet-host.nix
    # can set it without evaluation errors.
  };
}
