# tests/full-enrollment.nix — NixOS VM test: full enrollment flow
#
# End-to-end test of the device enrollment lifecycle:
#
# 1. A stateful mock API server starts on the control plane node
# 2. The enrollment device boots with hearth-enrollment in headless mode
# 3. The TUI auto-advances through Welcome → Hardware → Network → Login (token-injected)
# 4. Enrollment is submitted to the mock API, which assigns a machine_id
# 5. The test script simulates admin approval via the mock API's test endpoint
# 6. The enrollment TUI detects approval and enters provisioning
# 7. The TUI partitions, formats, and mounts a real virtual disk (/dev/vdb)
# 8. Machine identity (machine-id + machine-token) is persisted to /mnt
# 9. nixos-install fails (no real closure), but all prior steps are verified
#
# Nodes:
#   - controlplane: runs the stateful mock API server
#   - enrollee: boots the real hearth-enrollment binary in headless mode
#
# Verifications:
#   - State file transitions (via /run/hearth/enrollment-state)
#   - GPT partition table with correct types (EF00 + 8300)
#   - Filesystem types and labels (vfat/boot, ext4/nixos)
#   - Mount points (/mnt, /mnt/boot)
#   - Machine identity persistence (/mnt/var/lib/hearth/machine-{id,token})
#   - Structured log events

{ pkgs, lib, hearth-enrollment, ... }:

let
  mockApi = import ./lib/mock-api.nix { inherit pkgs; };
  enrollmentPkg = hearth-enrollment;
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

    # ──── Phase 4: Simulate admin approval ────
    controlplane.succeed(
        f"curl -sf -X POST http://localhost:3000/api/v1/test/approve/{machine_id}"
    )

    # ──── Phase 5: Wait for provisioning ────
    enrollee.wait_until_succeeds(
        "cat /run/hearth/enrollment-state | grep -q 'provisioning'", timeout=60
    )
    enrollee.screenshot("02-provisioning-started")

    # ──── Phase 6: Verify disk partitioning ────
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

    enrollee.screenshot("03-disk-partitioned-and-mounted")

    # ──── Phase 7: Verify machine identity persistence ────
    # Machine identity is written to /mnt/var/lib/hearth/ before nixos-install.
    enrollee.wait_until_succeeds("test -d /mnt/var/lib/hearth", timeout=60)
    enrollee.succeed("test -f /mnt/var/lib/hearth/machine-id")
    enrollee.succeed("test -f /mnt/var/lib/hearth/machine-token")

    # Verify the persisted machine-id matches the one assigned by the API
    written_id = enrollee.succeed("cat /mnt/var/lib/hearth/machine-id").strip()
    assert written_id == machine_id, (
        f"Machine ID mismatch: written={written_id}, expected={machine_id}"
    )

    # Verify machine-token is non-empty
    token = enrollee.succeed("cat /mnt/var/lib/hearth/machine-token").strip()
    assert len(token) > 0, "machine-token is empty"

    enrollee.screenshot("04-identity-persisted")

    # ──── Phase 8: Verify structured logs ────
    enrollee.succeed("grep -q 'screen_transition' /tmp/hearth-enrollment.log")

    enrollee.screenshot("05-final")
  '';
}
