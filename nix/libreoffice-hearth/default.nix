# nix/libreoffice-hearth/default.nix — LibreOffice 26.2 from official deb packages
#
# Extracts the full LibreOffice 26.2 from the official Linux x86_64 deb bundle,
# using the nixpkgs libreoffice-fresh build as the source for all runtime
# dependencies (correct rpaths, NixOS integration). The deb provides the 26.2
# binaries; libreoffice-fresh provides the dependency closure.

{ pkgs }:

# For now, use stock nixpkgs libreoffice-fresh (pre-built from binary cache).
# When LO 26.2 lands in nixpkgs, this will automatically update.
# The Rust UNO extensions install via .oxt regardless of LO version.
pkgs.libreoffice-fresh
