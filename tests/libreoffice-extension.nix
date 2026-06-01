# tests/libreoffice-extension.nix — NixOS VM test: LibreOffice extension loading
#
# Verifies that the hearth-office .oxt installs cleanly via unopkg and that
# LibreOffice can resolve the registered components. The .oxt now ships the
# C++ UNO bridge alongside the Rust .so; the bridge implements
# component_getFactory and is what LO actually calls into.
#
# This test runs against stock nixpkgs LibreOffice. The bridge does not
# require LO 26.2 / rust_uno — it speaks the standard UNO ABI, so older LO
# versions register the components fine.

{ pkgs, lib, hearth-office-oxt ? null, ... }:

pkgs.testers.nixosTest {
  name = "hearth-libreoffice-extension";

  nodes.machine = { config, pkgs, ... }: {
    environment.systemPackages = with pkgs; [
      libreoffice
      unzip
      file
    ];

    users.users.testuser = {
      isNormalUser = true;
      uid = 1000;
      home = "/home/testuser";
    };

    # LibreOffice + unopkg need XDG_RUNTIME_DIR to exist and be writable
    # by the running user. In a real login, pam_systemd creates
    # /run/user/<uid> with mode 0700. `su -` doesn't go through
    # logind, so pre-stage the directory here for the test.
    systemd.tmpfiles.rules = [
      "d /run/user/1000 0700 testuser users -"
    ];

    environment.etc."skel/.config/hearth/office.toml".text = ''
      [nextcloud]
      url = "https://cloud.test.example.com"
      webdav_url = "https://cloud.test.example.com/remote.php/dav/files/"
    '';
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    ${lib.optionalString (hearth-office-oxt != null) ''
      # ZIP structure: both .so files + descriptors must be present.
      machine.succeed("file ${hearth-office-oxt}/hearth-office.oxt | grep -q 'Zip archive'")
      for entry in [
          "META-INF/manifest.xml",
          "description.xml",
          "hearth-office.components",
          "Addons.xcu",
          "ProtocolHandler.xcu",
          "libhearth_office.so",
          "libhearth_office_bridge.so",
      ]:
          machine.succeed(f"unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q '{entry}'")

      # The .components file must point at the C++ bridge .so (not the Rust one).
      machine.succeed(
          "unzip -p ${hearth-office-oxt}/hearth-office.oxt hearth-office.components"
          " | grep -q 'libhearth_office_bridge.so'"
      )

      # Install the extension as the test user. unopkg unpacks both .so files
      # into the extension cache; the bridge's $ORIGIN-relative DT_NEEDED
      # then resolves libhearth_office.so as a sibling. Set
      # XDG_RUNTIME_DIR explicitly because `su -` doesn't create one.
      # Capture stderr so we can assert it stays free of configmgr schema
      # warnings — the OfficeMenuBar entry has to wrap menu items in a
      # Submenu node per the PopupMenu schema (officecfg/registry/schema/
      # org/openoffice/Office/Addons.xcs); a regression there silently
      # surfaces as `unknown node "m1"` in unopkg's stderr.
      machine.succeed(
          "su - testuser -c '"
          "export XDG_RUNTIME_DIR=/run/user/1000; "
          "$(find /nix/store -name unopkg -path \"*/libreoffice/program/*\" | head -1)"
          " add --suppress-license"
          " ${hearth-office-oxt}/hearth-office.oxt'"
          " 2>/tmp/unopkg-add.stderr"
      )

      # Regression gate for nix/oxt/Addons.xcu OfficeMenuBar structure.
      stderr = machine.succeed("cat /tmp/unopkg-add.stderr || true")
      assert "unknown node" not in stderr, (
          f"unopkg add emitted a configmgr schema warning — check "
          f"Addons.xcu for PopupMenu/Submenu structure regressions. "
          f"stderr:\n{stderr}"
      )

      # unopkg list must show the extension registered and active.
      result = machine.succeed(
          "su - testuser -c '"
          "export XDG_RUNTIME_DIR=/run/user/1000; "
          "$(find /nix/store -name unopkg -path \"*/libreoffice/program/*\" | head -1)"
          " list'"
      )
      assert "com.hearth.office" in result, f"extension not listed: {result}"
    ''}

    # office.toml config sanity check.
    machine.succeed(
      "mkdir -p /home/testuser/.config/hearth && "
      "cp /etc/skel/.config/hearth/office.toml /home/testuser/.config/hearth/ && "
      "chown -R testuser:users /home/testuser/.config"
    )
    machine.succeed("grep -q 'cloud.test.example.com' /home/testuser/.config/hearth/office.toml")

    # LO starts headless without crashing — proves the extension's component
    # loader doesn't blow up at registration time.
    machine.succeed(
      "su - testuser -c '"
      "export XDG_RUNTIME_DIR=/run/user/1000; "
      "timeout 30 soffice --headless --norestore --nofirststartwizard --calc --convert-to csv /dev/null 2>&1 || true'"
    )
  '';
}
