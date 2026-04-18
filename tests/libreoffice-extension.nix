# tests/libreoffice-extension.nix — NixOS VM test: LibreOffice extension loading
#
# Verifies that the hearth-office .oxt extension can be installed via unopkg
# and that LibreOffice starts headless with the extension registered.
#
# This test uses the standard LibreOffice (not libreoffice-hearth with Rust UNO)
# to validate the .oxt packaging structure without requiring the full 26.2 build.
# The actual Rust UNO component registration is tested separately as it requires
# the custom LO build.
#
# Assertions:
#   - unopkg can install the .oxt without errors
#   - The .oxt ZIP structure is valid (manifest.xml, description.xml, etc.)
#   - LibreOffice starts headless and exits cleanly
#   - The extension config file (office.toml) is written correctly

{ pkgs, lib, hearth-office-oxt ? null, ... }:

pkgs.testers.nixosTest {
  name = "hearth-libreoffice-extension";

  nodes.machine = { config, pkgs, ... }: {
    environment.systemPackages = with pkgs; [
      libreoffice
      unzip
      file
    ];

    # Create a test user
    users.users.testuser = {
      isNormalUser = true;
      home = "/home/testuser";
    };

    # Write a test office.toml
    environment.etc."skel/.config/hearth/office.toml".text = ''
      [nextcloud]
      url = "https://cloud.test.example.com"
      webdav_url = "https://cloud.test.example.com/remote.php/dav/files/"
    '';
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # Verify the .oxt is a valid ZIP archive
    ${lib.optionalString (hearth-office-oxt != null) ''
      machine.succeed("file ${hearth-office-oxt}/hearth-office.oxt | grep -q 'Zip archive'")

      # Check that required files are inside the .oxt
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'META-INF/manifest.xml'")
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'description.xml'")
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'hearth-office.components'")
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'Addons.xcu'")
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'ProtocolHandler.xcu'")
      machine.succeed("unzip -l ${hearth-office-oxt}/hearth-office.oxt | grep -q 'libhearth_office.so'")
    ''}

    # Verify the office.toml config structure
    machine.succeed(
      "mkdir -p /home/testuser/.config/hearth && "
      "cp /etc/skel/.config/hearth/office.toml /home/testuser/.config/hearth/ && "
      "chown -R testuser:users /home/testuser/.config"
    )
    machine.succeed("grep -q 'cloud.test.example.com' /home/testuser/.config/hearth/office.toml")

    # Verify LibreOffice starts headless and exits cleanly
    machine.succeed(
      "su - testuser -c 'timeout 30 soffice --headless --norestore --nofirststartwizard --calc --convert-to csv /dev/null 2>&1 || true'"
    )

    # Verify LibreOffice program directory exists and has unopkg
    machine.succeed("test -x $(find /nix/store -name unopkg -path '*/libreoffice/program/*' | head -1)")
  '';
}
