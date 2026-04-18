# nix/libreoffice-uno-libs.nix — Extract UNO shared libraries from LibreOffice 26.2 debs
#
# Downloads the official LO 26.2 Linux x86_64 deb bundle and extracts only the
# UNO runtime libraries needed for linking hearth-office and rust_uno:
#   libuno_cppu.so, libuno_sal.so, libuno_salhelpergcc3.so, librust_uno-cpplo.so
#
# This avoids building LibreOffice from source (~3 hours) while providing the
# symbols needed for development and CI.

{ pkgs
, stdenv ? pkgs.stdenv
}:

let
  deb-tarball = pkgs.fetchurl {
    url = "https://download.documentfoundation.org/libreoffice/stable/26.2.2/deb/x86_64/LibreOffice_26.2.2_Linux_x86-64_deb.tar.gz";
    sha256 = "1s3283bha7l7bp4q247f3aa7h79g4cj3xi26wh1dqibyx6piaa5a";
  };
in
pkgs.runCommand "libreoffice-uno-libs" {
  nativeBuildInputs = with pkgs; [ gnutar xz dpkg patchelf ];
  buildInputs = with pkgs; [ zlib gcc.cc.lib ];
} ''
  mkdir -p work
  tar -xzf ${deb-tarball} -C work --strip-components=1

  # Extract all .deb files to find UNO libraries
  mkdir -p extracted
  for deb in work/DEBS/*.deb; do
    dpkg-deb -x "$deb" extracted/ 2>/dev/null || true
  done

  # Layout matches what rust_uno's build.rs expects: $INSTDIR/program/
  mkdir -p $out/program $out/lib
  find extracted -name "libuno_cppu.so*" -exec cp -P {} $out/program/ \;
  find extracted -name "libuno_sal.so*" -exec cp -P {} $out/program/ \;
  find extracted -name "libuno_salhelpergcc3.so*" -exec cp -P {} $out/program/ \;
  find extracted -name "librust_uno*" -exec cp -P {} $out/program/ \;
  find extracted -name "libuno_cppuhelpergcc3.so*" -exec cp -P {} $out/program/ \;
  find extracted -name "libunsafe_uno_uno.so*" -exec cp -P {} $out/program/ \;

  # Create unversioned symlinks for the linker (libs are versioned like .so.3)
  cd $out/program
  for lib in *.so.*; do
    base=$(echo "$lib" | sed 's/\.so\..*/\.so/')
    [ ! -e "$base" ] && ln -s "$lib" "$base" || true
  done

  # Also symlink into lib/ for standard library path resolution
  cd $out/lib
  for lib in ../program/*.so*; do
    ln -sf "$lib" . 2>/dev/null || true
  done

  # Patch the extracted libs to use Nix store versions of runtime deps
  # (libstdc++, libz, libm, etc. — the debs were built against Ubuntu)
  for lib in $out/program/*.so $out/program/*.so.*; do
    [ -L "$lib" ] && continue  # skip symlinks
    [ -f "$lib" ] || continue
    patchelf --set-rpath "${pkgs.lib.makeLibraryPath [ pkgs.zlib pkgs.gcc.cc.lib pkgs.glibc ]}" "$lib" 2>/dev/null || true
  done

  echo "=== UNO libraries extracted ==="
  ls -la $out/program/
''
