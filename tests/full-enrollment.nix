# tests/full-enrollment.nix — NixOS VM test: full enrollment flow
#
# End-to-end test of the complete device enrollment lifecycle:
#
# 1. A stateful mock API server starts on the control plane node
# 2. The enrollment device boots with hearth-enrollment in headless mode
# 3. The TUI auto-advances: Welcome → Hardware → Network → Login → Enroll → Status
# 4. Enrollment request is submitted — test verifies request body contents
# 5. Test verifies machine appears as "pending" before approval
# 6. Admin approval is simulated via the mock API's test endpoint
# 7. Enrollment TUI detects approval and enters provisioning
# 8. TUI partitions, formats, and mounts a real virtual disk (/dev/vdb)
# 9. Machine identity is persisted; test verifies token matches API response
# 10. nixos-install runs with a REAL pre-built NixOS closure and succeeds
# 11. Installed system is verified (os-release, system profile, identity files)
#
# Nodes:
#   - controlplane: runs the stateful mock API server
#   - enrollee: boots the real hearth-enrollment binary in headless mode
#
# Verifications:
#   - Enrollment request body content (hostname, hardware_fingerprint, hardware_report)
#   - Machine pending status before approval
#   - State file transitions (via /run/hearth/enrollment-state)
#   - GPT partition table with correct types (EF00 + 8300)
#   - Filesystem types and labels (vfat/boot, ext4/nixos)
#   - Mount points (/mnt, /mnt/boot)
#   - Machine identity persistence with token consistency
#   - nixos-install success with real NixOS closure
#   - Installed system integrity (os-release, system profile)
#   - Structured log events

{ pkgs, lib, hearth-enrollment, hearth-agent, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  enrollmentPkg = hearth-enrollment;

  # Build a minimal NixOS system closure that can be installed onto the
  # provisioned disk. This exercises the real nixos-install path.
  # Includes hearth-agent so future tests can verify post-install agent boot.
  minimalTarget = (pkgs.nixos ({ lib, pkgs, ... }: {
    nixpkgs.overlays = [
      (final: prev: {
        hearth-agent = hearth-agent;
      })
    ];

    imports = [ ../modules/agent.nix ];

    # Must match the partition layout created by the enrollment TUI
    boot.loader.grub.device = "/dev/vdb";
    boot.loader.grub.enable = true;
    boot.loader.systemd-boot.enable = false;
    fileSystems."/" = { device = "/dev/vdb2"; fsType = "ext4"; };
    fileSystems."/boot" = { device = "/dev/vdb1"; fsType = "vfat"; };

    # Hearth agent configured to poll the control plane
    services.hearth.agent = {
      enable = true;
      serverUrl = "http://controlplane:3000";
      machineId = "";  # Read from /var/lib/hearth/machine-id at runtime
      pollInterval = 5;
    };

    # Minimal system — no desktop, no greeter
    services.openssh.enable = true;
    networking.hostName = "enrolled-machine";
    system.stateVersion = "25.05";
  })).config.system.build.toplevel;
in
pkgs.testers.nixosTest {
  name = "hearth-full-enrollment";

  nodes = {
    controlplane = { config, pkgs, ... }: {
      imports = [ (mockApi.module { port = 3000; }) ];
    };

    enrollee = { config, lib, pkgs, ... }: {
      imports = [ ../modules/enrollment.nix ];

      nixpkgs.overlays = [
        (final: prev: {
          hearth-enrollment = enrollmentPkg;
        })
      ];

      services.hearth.enrollment = {
        enable = true;
        serverUrl = "http://controlplane:3000";
        wifiSupport = false;
        # No kanidmUrl — use token injection to skip browser auth entirely
      };

      # Override enrollment module defaults for test VM
      hardware.enableAllHardware = lib.mkForce false;
      # The test framework derives Python variable names from the VM hostname
      # (via regex on the start script path). Override the enrollment module's
      # default hostname so the Python variable matches our node attrset key.
      networking.hostName = lib.mkForce "enrollee";

      # Headless mode + token injection + target disk
      environment.variables = {
        HEARTH_HEADLESS = "1";
        HEARTH_AUTH_TOKEN = "test-enrollment-token";
        HEARTH_TARGET_DISK = "vdb";
      };

      # Extra virtual disk for provisioning target
      virtualisation = {
        memorySize = 2048;
        emptyDiskImages = [ 8192 ]; # 8GB disk at /dev/vdb
      };

      # Make the target system closure available in the VM's Nix store
      # so nixos-install can copy it to the provisioned disk.
      virtualisation.additionalPaths = [ minimalTarget ];

      # Enrollment needs disk utilities available
      environment.systemPackages = with pkgs; [
        gptfdisk    # sgdisk
        e2fsprogs   # mkfs.ext4
        dosfstools  # mkfs.fat
        util-linux  # lsblk, mount, etc.
        curl
      ];
    };
  };

  testScript = ''
    import json

    # ──── Phase 1: Boot infrastructure ────
    controlplane.start()
    controlplane.wait_for_unit("hearth-mock-api.service")
    controlplane.wait_for_open_port(3000)

    # Verify mock API is healthy
    controlplane.succeed("curl -sf http://localhost:3000/health")

    # ──── Phase 2: Boot enrollment device ────
    enrollee.start()
    enrollee.wait_for_unit("multi-user.target")

    # Verify enrollment config was generated
    enrollee.succeed("test -f /etc/hearth/enrollment.toml")

    # Verify the real enrollment binary is available
    enrollee.succeed("which hearth-enrollment")

    # Verify the device can reach the control plane
    enrollee.succeed("curl -sf http://controlplane:3000/health")

    # ──── Phase 3: Wait for enrollment to auto-advance ────
    # The enrollment TUI runs in headless mode and writes state to
    # /run/hearth/enrollment-state as it transitions through screens.

    # Wait for the enrollment state file to appear
    enrollee.wait_until_succeeds("test -f /run/hearth/enrollment-state", timeout=60)

    # Wait for enrollment to reach the status screen (waiting for approval)
    enrollee.wait_until_succeeds(
        "cat /run/hearth/enrollment-state | grep -q 'status'", timeout=120
    )
    enrollee.screenshot("01-waiting-for-approval")

    # Verify machine_id was written
    enrollee.wait_until_succeeds("test -f /run/hearth/enrollment-machine-id", timeout=10)
    machine_id = enrollee.succeed("cat /run/hearth/enrollment-machine-id").strip()
    assert len(machine_id) > 0, "machine_id is empty"

    # ──── Phase 4: Verify enrollment request content ────
    # Query the mock API's introspection endpoint to verify what was sent
    enrollment_raw = controlplane.succeed(
        f"curl -sf http://localhost:3000/api/v1/test/enrollments/{machine_id}"
    )
    enrollment_data = json.loads(enrollment_raw)
    assert enrollment_data["hostname"] == "enrollee", (
        f"Expected hostname 'enrollee', got '{enrollment_data['hostname']}'"
    )
    assert enrollment_data.get("hardware_fingerprint") is not None, (
        "hardware_fingerprint should be present in enrollment request"
    )
    assert enrollment_data.get("hardware_report") is not None, (
        "hardware_report should be present in enrollment request"
    )

    # Verify machine is pending before approval
    status_raw = controlplane.succeed(
        f"curl -sf http://localhost:3000/api/v1/machines/{machine_id}/enrollment-status"
    )
    status_data = json.loads(status_raw)
    assert status_data["status"] == "pending", (
        f"Expected status 'pending' before approval, got '{status_data['status']}'"
    )

    enrollee.screenshot("02-enrollment-verified-pending")

    # ──── Phase 5: Simulate admin approval with real closure ────
    controlplane.succeed(
        "curl -sf -X POST http://localhost:3000/api/v1/test/approve/"
        + machine_id
        + " -H 'Content-Type: application/json'"
        + " -d '{\"target_closure\": \"${minimalTarget}\"}'"
    )

    # ──── Phase 6: Wait for provisioning ────
    enrollee.wait_until_succeeds(
        "cat /run/hearth/enrollment-state | grep -q 'provisioning'", timeout=60
    )
    enrollee.screenshot("03-provisioning-started")

    # ──── Phase 7: Verify disk partitioning ────
    # The enrollment TUI runs real sgdisk + mkfs against /dev/vdb.
    # Wait for partitions to be created.
    enrollee.wait_until_succeeds("lsblk /dev/vdb1 2>/dev/null", timeout=120)
    enrollee.wait_until_succeeds("lsblk /dev/vdb2 2>/dev/null", timeout=30)

    # Verify GPT partition types
    enrollee.succeed("sgdisk -p /dev/vdb | grep -q 'EF00'")   # EFI System Partition
    enrollee.succeed("sgdisk -p /dev/vdb | grep -q '8300'")   # Linux filesystem

    # Verify filesystem types and labels
    enrollee.succeed("blkid /dev/vdb1 | grep -q 'TYPE=\"vfat\"'")
    enrollee.succeed("blkid /dev/vdb1 | grep -q 'LABEL=\"boot\"'")
    enrollee.succeed("blkid /dev/vdb2 | grep -q 'TYPE=\"ext4\"'")
    enrollee.succeed("blkid /dev/vdb2 | grep -q 'LABEL=\"nixos\"'")

    # Verify mount points
    enrollee.succeed("mountpoint -q /mnt")
    enrollee.succeed("mountpoint -q /mnt/boot")

    enrollee.screenshot("04-disk-partitioned-and-mounted")

    # ──── Phase 8: Verify machine identity persistence ────
    # Machine identity is written to /mnt/var/lib/hearth/ before nixos-install.
    enrollee.wait_until_succeeds("test -d /mnt/var/lib/hearth", timeout=60)
    enrollee.succeed("test -f /mnt/var/lib/hearth/machine-id")
    enrollee.succeed("test -f /mnt/var/lib/hearth/machine-token")

    # Verify the persisted machine-id matches the one assigned by the API
    written_id = enrollee.succeed("cat /mnt/var/lib/hearth/machine-id").strip()
    assert written_id == machine_id, (
        f"Machine ID mismatch: written={written_id}, expected={machine_id}"
    )

    # Verify machine-token matches what the mock API would return
    expected_token = f"test-machine-token-{machine_id}"
    actual_token = enrollee.succeed("cat /mnt/var/lib/hearth/machine-token").strip()
    assert actual_token == expected_token, (
        f"Machine token mismatch: written={actual_token}, expected={expected_token}"
    )

    enrollee.screenshot("05-identity-persisted")

    # ──── Phase 9: Verify nixos-install success ────
    # The enrollment TUI runs nixos-install with the real pre-built closure.
    # Wait for the installed system to appear on the provisioned disk.
    enrollee.wait_until_succeeds("test -f /mnt/etc/os-release", timeout=300)
    enrollee.succeed("test -L /mnt/nix/var/nix/profiles/system")

    enrollee.screenshot("06-install-complete")

    # ──── Phase 10: Verify structured logs ────
    enrollee.succeed("grep -q 'screen_transition' /tmp/hearth-enrollment.log")

    enrollee.screenshot("07-final")
  '';
}
