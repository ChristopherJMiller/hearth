# overlays/default.nix — Overlay exposing Hearth packages into nixpkgs
#
# This overlay adds all Hearth-built packages to any nixpkgs instance.
# Fleet machines apply this overlay so that NixOS modules can reference
# pkgs.hearth-agent, pkgs.hearth-greeter, etc. without explicit package
# path references.
#
# Usage in a flake:
#   nixpkgs.overlays = [ hearth.overlays.default ];
#
# The overlay receives the hearth flake's self reference via a wrapper
# in flake.nix. The packages are built for the target system.
{ self }:

final: prev: {
  # Core Hearth packages
  hearth-agent = self.packages.${prev.system}.hearth-agent;
  hearth-greeter = self.packages.${prev.system}.hearth-greeter;
  hearth-enrollment = self.packages.${prev.system}.hearth-enrollment;
  hearth-api = self.packages.${prev.system}.hearth-api;

  # Convenience alias for all Hearth packages
  hearth-packages = prev.symlinkJoin {
    name = "hearth-packages";
    paths = [
      final.hearth-agent
      final.hearth-greeter
      final.hearth-enrollment
      final.hearth-api
    ];
  };
}
