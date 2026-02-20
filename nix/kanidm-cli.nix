# nix/kanidm-cli.nix — Kanidm 1.9 CLI client for dev use
#
# Builds only the `kanidm` CLI binary (tools/cli) from the kanidm workspace.
# Used in the dev shell for running bootstrap.sh and managing the dev Kanidm instance.
#
# Requires a rustPlatform with Rust >= 1.93 (kanidm 1.9 MSRV).
# To update: bump version, set hashes to lib.fakeHash, build to get real hashes.

{ lib
, stdenv
, makeRustPlatform
, rust-bin
, fetchFromGitHub
, formats
, pkg-config
, openssl
, sqlite
, pam
, udev
, bashInteractive
}:

let
  version = "1.9.0";
  arch = if stdenv.hostPlatform.isx86_64 then "x86_64" else "generic";
  buildProfile = "release_nixpkgs_${arch}";

  # Kanidm 1.9 requires Rust 1.93+
  rustToolchain = rust-bin.stable."1.93.0".minimal;
  rustPlatform = makeRustPlatform {
    cargo = rustToolchain;
    rustc = rustToolchain;
  };
in
rustPlatform.buildRustPackage {
  pname = "kanidm-cli";
  inherit version;

  src = fetchFromGitHub {
    owner = "kanidm";
    repo = "kanidm";
    rev = "refs/tags/v${version}";
    hash = "sha256-PAYD+CSvDVtx5SFRtTogbu7Az+9WFVeFL/76Dr/pOog=";
  };

  cargoHash = "sha256-razlbe5VEiWz427dShvWT/rVuvBh5Re/z1vXsVQGOgM=";

  env.KANIDM_BUILD_PROFILE = buildProfile;
  env.RUSTFLAGS = "--cap-lints warn";

  postPatch = let
    format = (formats.toml { }).generate "${buildProfile}.toml";
    profile = {
      cpu_flags = if stdenv.hostPlatform.isx86_64 then "x86_64_legacy" else "none";
      client_config_path = "/etc/kanidm/config";
      resolver_config_path = "/etc/kanidm/unixd";
      resolver_unix_shell_path = "${lib.getBin bashInteractive}/bin/bash";
      resolver_service_account_token_path = "/etc/kanidm/token";
      server_admin_bind_path = "/run/kanidmd/sock";
      server_config_path = "/etc/kanidm/server.toml";
      server_migration_path = "/etc/kanidm/migrations.d";
      server_ui_pkg_path = "@htmx_ui_pkg_path@";
    };
  in ''
    cp ${format profile} libs/profiles/${buildProfile}.toml
    substituteInPlace libs/profiles/${buildProfile}.toml \
      --replace-fail '@htmx_ui_pkg_path@' "$out/ui/hpkg"
  '';

  # Only build the CLI binary
  cargoBuildFlags = [ "--package" "kanidm_tools" ];

  # Skip tests — we just need the binary for dev
  doCheck = false;

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [
    openssl
    sqlite
    pam
  ] ++ lib.optionals stdenv.hostPlatform.isLinux [
    udev
  ];

  # The server UI assets are needed by the build profile but we
  # don't ship them — create a placeholder so the build profile resolves.
  postBuild = ''
    mkdir -p $out/ui/hpkg
  '';

  meta = {
    description = "Kanidm CLI client";
    homepage = "https://github.com/kanidm/kanidm";
    license = lib.licenses.mpl20;
    platforms = lib.platforms.linux;
  };
}
