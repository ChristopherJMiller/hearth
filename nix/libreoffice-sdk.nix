# nix/libreoffice-sdk.nix — LibreOffice 26.2 SDK headers + generated UNO C++ bindings
#
# Produces what's needed to compile a C++ UNO extension against LO 26.2:
#   $out/sdk/include/cppu/, cppuhelper/, osl/, rtl/, sal/, salhelper/, typelib/, uno/
#       (helper headers from the SDK deb)
#   $out/sdk/include/com/sun/star/...                (generated from offapi.rdb via cppumaker)
#
# The SDK deb ships only helper headers; com::sun::star type headers must be
# generated from .rdb registry files using cppumaker. cppumaker itself needs
# libunoidllo.so + UNO runtime libs from the URE deb.

{ pkgs }:

let
  sdk-tarball = pkgs.fetchurl {
    url = "https://download.documentfoundation.org/libreoffice/stable/26.2.2/deb/x86_64/LibreOffice_26.2.2_Linux_x86-64_deb_sdk.tar.gz";
    sha256 = "17vcj3m1r4grznnyi66qkqh9r3ny5w28jxwz06m2jz8hqqrpj881";
  };
  main-tarball = pkgs.fetchurl {
    # Same tarball libreoffice-uno-libs.nix uses; Nix dedupes.
    url = "https://download.documentfoundation.org/libreoffice/stable/26.2.2/deb/x86_64/LibreOffice_26.2.2_Linux_x86-64_deb.tar.gz";
    sha256 = "1s3283bha7l7bp4q247f3aa7h79g4cj3xi26wh1dqibyx6piaa5a";
  };
in
pkgs.runCommand "libreoffice-sdk-26.2" {
  nativeBuildInputs = with pkgs; [ gnutar gzip dpkg patchelf ];
  buildInputs = with pkgs; [ zlib gcc.cc.lib glibc ];
} ''
  mkdir -p sdk-extract main-extract

  # SDK deb tarball (~21 MB) — provides cppumaker + helper headers
  tar -xzf ${sdk-tarball} -C sdk-extract --strip-components=1
  mkdir -p sdk-out
  for deb in sdk-extract/DEBS/*.deb; do
    dpkg-deb -x "$deb" sdk-out/ 2>/dev/null || true
  done

  # Main deb tarball (~256 MB) — provides URE (libunoidllo.so + types.rdb) and
  # offapi.rdb. Only the URE + core debs are extracted to keep the closure small.
  tar -xzf ${main-tarball} -C main-extract --strip-components=1
  mkdir -p main-out
  ls main-extract/DEBS/ | grep -E '(-ure_|-core_)' | head
  for deb in main-extract/DEBS/libreoffice26.2-ure_*.deb \
             main-extract/DEBS/libobasis26.2-core_*.deb; do
    echo "Extracting: $deb"
    dpkg-deb -x "$deb" main-out/
  done
  echo "After extraction, types.rdb status:"
  find main-out -name types.rdb -exec ls -la {} \;
  find main-out -name offapi.rdb -exec ls -la {} \;

  # Locate paths in the extracted trees (use absolute paths — cppumaker
  # internally uses some bootstrap logic that's sensitive to relative paths)
  sdk_root=$(realpath $(find sdk-out -type d -name sdk -path '*libreoffice*' | head -1))
  ure_program=$(realpath $(find main-out -type d -path '*libreoffice26.2/program' | head -1))
  core_types=$(realpath $(find main-out -type f -name offapi.rdb | head -1))

  if [ -z "$sdk_root" ] || [ -z "$ure_program" ] || [ -z "$core_types" ]; then
    echo "ERROR: missing required SDK/URE/core paths"
    echo "  sdk_root=$sdk_root"
    echo "  ure_program=$ure_program"
    echo "  core_types=$core_types"
    exit 1
  fi

  mkdir -p $out
  cp -r "$sdk_root" $out/sdk

  # Patch cppumaker (and any extracted ELFs) so they find their UNO deps in
  # $ure_program. cppumaker itself + its libunoidllo.so were built against
  # Ubuntu glibc; rewrite RPATH to use Nix store glibc + libstdc++.
  rt_libs="${pkgs.lib.makeLibraryPath [ pkgs.zlib pkgs.gcc.cc.lib pkgs.glibc ]}"

  ld_linux="${pkgs.glibc}/lib/ld-linux-x86-64.so.2"
  for bin in $out/sdk/bin/cppumaker $out/sdk/bin/idlc $out/sdk/bin/regmerge \
             $out/sdk/bin/unoidl-read $out/sdk/bin/unoidl-write; do
    [ -f "$bin" ] && patchelf \
      --set-interpreter "$ld_linux" \
      --set-rpath "$ure_program:$rt_libs" "$bin" 2>/dev/null || true
  done

  for so in "$ure_program"/*.so "$ure_program"/*.so.*; do
    [ -L "$so" ] && continue
    [ -f "$so" ] || continue
    patchelf --set-rpath "$ure_program:$rt_libs" "$so" 2>/dev/null || true
  done

  # Generate com::sun::star::* C++ headers from the type registries.
  # Lightweight mode (-L) — bridge code uses the public API only.
  mkdir -p $out/sdk/include
  echo "Running cppumaker (this generates ~thousands of .hpp files)..."
  LD_LIBRARY_PATH="$ure_program" \
    $out/sdk/bin/cppumaker \
      -O$out/sdk/include \
      -L \
      "$ure_program/types.rdb" \
      "$core_types"

  echo "=== SDK ready ==="
  echo "Helper headers:"
  ls $out/sdk/include | grep -v "^com$" | head
  echo "Generated com/sun/star headers (sample):"
  find $out/sdk/include/com/sun/star -name "XDispatchProvider.hpp" -o -name "XStatusbarController.hpp" -o -name "XUIElementFactory.hpp" 2>/dev/null
''
