# nix/hearth-office-bridge.nix — C++ UNO shim that wires LibreOffice's
# component framework to the Rust hearth-office .so. Compiles a single
# libhearth_office_bridge.so against the LO 26.2 SDK and links against
# libhearth_office.so via $ORIGIN-relative RPATH so both .so files
# resolve each other after unopkg unpacks the .oxt.

{ pkgs
, hearth-office-so
, libreoffice-sdk
, libreoffice-uno-libs
}:

pkgs.stdenv.mkDerivation {
  pname = "hearth-office-bridge";
  version = "0.1.0";

  src = ../cpp/hearth-office-bridge;

  nativeBuildInputs = with pkgs; [ patchelf ];

  buildPhase = ''
    runHook preBuild

    SDK_INCLUDE="${libreoffice-sdk}/sdk/include"
    UNO_LIBS="${libreoffice-uno-libs}/program"
    RUST_LIB="${hearth-office-so}/lib"

    mkdir -p build

    # Per-source compilation. Visibility is hidden by default; only the
    # SAL_DLLPUBLIC_EXPORT functions in bridge.cxx need to escape.
    sources="bridge.cxx frame_url.cxx share_handler.cxx \
             lock_status_controller.cxx comments_panel.cxx"
    objs=""
    for src in $sources; do
      [ -f "$src" ] || continue
      obj="build/$(basename "$src" .cxx).o"
      echo "  CXX  $src"
      $CXX -c -fPIC -std=c++17 -O2 \
        -fvisibility=hidden -fvisibility-inlines-hidden \
        -DLINUX -DUNX -DGCC -DCPPU_ENV=gcc3 -DHAVE_GCC_VISIBILITY_FEATURE \
        -I"$SDK_INCLUDE" \
        "$src" -o "$obj"
      objs="$objs $obj"
    done

    echo "  LINK libhearth_office_bridge.so"
    $CXX -shared -fPIC \
      -Wl,-z,defs \
      -Wl,-rpath,'$ORIGIN' \
      -L"$UNO_LIBS" -L"$RUST_LIB" \
      -o build/libhearth_office_bridge.so \
      $objs \
      -luno_cppu -luno_sal -luno_cppuhelpergcc3 -luno_salhelpergcc3 \
      -lhearth_office

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out/lib
    cp build/libhearth_office_bridge.so $out/lib/
    runHook postInstall
  '';

  # The C++ shim's RPATH is intentionally `$ORIGIN` so it finds the sibling
  # libhearth_office.so when both are unpacked side-by-side from the .oxt.
  # Don't let stdenv rewrite that to absolute Nix paths.
  dontPatchELF = true;

  meta = with pkgs.lib; {
    description = "C++ UNO shim that bridges LibreOffice to libhearth_office.so";
    license = licenses.agpl3Plus;
    platforms = platforms.linux;
  };
}
