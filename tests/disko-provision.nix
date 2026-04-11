# tests/disko-provision.nix — NixOS VM test: disko config validation
#
# Validates that the disko configs bundled in the enrollment module are
# syntactically correct and that the disko CLI accepts the argument format
# used by the enrollment TUI's provisioning code.
#
# This test uses --dry-run to validate without actually partitioning,
# avoiding the need for a full Nix build environment in the VM.

{ pkgs, lib, hearth-enrollment ? null, ... }:

pkgs.testers.nixosTest {
  name = "hearth-disko-provision";

  nodes.machine = { config, lib, pkgs, ... }: {
    imports = [ ../modules/enrollment.nix ];

    nixpkgs.overlays = lib.optional (hearth-enrollment != null) (
      final: prev: { hearth-enrollment = hearth-enrollment; }
    );

    services.hearth.enrollment = {
      enable = true;
      serverUrl = "http://localhost:3000";
      wifiSupport = false;
    };

    hardware.enableAllHardware = lib.mkForce false;
    networking.hostName = lib.mkForce "machine";

    nix.nixPath = [ "nixpkgs=${pkgs.path}" ];
    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    virtualisation = {
      memorySize = 2048;
      emptyDiskImages = [ 8192 ]; # 8GB at /dev/vdb
    };

    environment.systemPackages = with pkgs; [
      disko
      gptfdisk
      e2fsprogs
      dosfstools
      util-linux
      parted
    ];
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # ──── Verify disko configs are bundled ────
    machine.succeed("test -f /etc/hearth/disko-configs/standard.nix")
    machine.succeed("test -f /etc/hearth/disko-configs/luks-lvm.nix")

    # ──── Validate disko CLI accepts the argument format ────
    # Use --dry-run to validate without needing stdenv in the VM store.
    # This catches argument ordering and flag name regressions.
    machine.succeed(
        "disko"
        " --dry-run"
        " --mode format,mount"
        " --argstr device /dev/vdb"
        " --no-deps"
        " /etc/hearth/disko-configs/standard.nix"
    )

    # ──── Validate the config is valid Nix ────
    machine.succeed(
        "nix-instantiate --eval --strict --json"
        " --argstr device /dev/vdb"
        " /etc/hearth/disko-configs/standard.nix"
        " > /dev/null"
    )
    machine.succeed(
        "nix-instantiate --eval --strict --json"
        " --argstr device /dev/vdb"
        " /etc/hearth/disko-configs/luks-lvm.nix"
        " > /dev/null"
    )
  '';
}
