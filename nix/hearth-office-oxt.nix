# nix/hearth-office-oxt.nix — Package hearth-office as a LibreOffice .oxt extension
#
# Combines the compiled Rust cdylib (.so) with the XML descriptors and icons
# into a ZIP archive with the .oxt extension, ready for installation via unopkg.
#
# Usage:
#   hearth-office-oxt = pkgs.callPackage ./nix/hearth-office-oxt.nix {
#     inherit hearth-office-so;
#   };

{ pkgs
, hearth-office-so
, hearth-office-bridge
}:

pkgs.runCommand "hearth-office-oxt" {
  nativeBuildInputs = [ pkgs.zip ];
} ''
  mkdir -p oxt/META-INF oxt/icons

  # Copy the Rust UNO shared library (business logic) and the C++ bridge .so
  # (LibreOffice's component entry point). The bridge's RPATH is $ORIGIN, so
  # both must land in the same directory inside the .oxt.
  cp ${hearth-office-so}/lib/libhearth_office.so oxt/
  cp ${hearth-office-bridge}/lib/libhearth_office_bridge.so oxt/

  # Copy extension descriptors
  cp ${../nix/oxt/META-INF/manifest.xml} oxt/META-INF/manifest.xml
  cp ${../nix/oxt/description.xml} oxt/description.xml
  cp ${../nix/oxt/hearth-office.components} oxt/hearth-office.components
  cp ${../nix/oxt/Addons.xcu} oxt/Addons.xcu
  cp ${../nix/oxt/ProtocolHandler.xcu} oxt/ProtocolHandler.xcu

  # Copy icons (skip README)
  for icon in ${../nix/oxt/icons}/*.png; do
    [ -f "$icon" ] && cp "$icon" oxt/icons/ || true
  done

  # Create the .oxt archive (ZIP format)
  mkdir -p $out
  cd oxt && zip -r $out/hearth-office.oxt .
''
