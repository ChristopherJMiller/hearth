# tests/firefox-module-eval.nix — Pure-Nix eval test for home-modules/firefox.nix
#
# Validates the policy-rendering logic in the Firefox home-manager module
# without booting a VM or installing Firefox. We construct a synthetic
# module set with the home-modules/firefox module + a stub that declares
# `programs.firefox.policies` as a freeform attrset so we can pluck out
# the rendered policy and assert on its structure.
#
# Wired into flake.nix as a check; runs in well under a second.

{ pkgs, lib ? pkgs.lib, ... }:

let
  firefoxModule = import ../home-modules/firefox.nix;

  # Minimal stubs for the home-manager option surface our module touches.
  # We don't need home-manager itself — just the option declarations so
  # evalModules accepts our `programs.firefox.policies = ...;` setting.
  # Use lib.types.anything so nested `lib.mkIf` values inside the policies
  # attrset (e.g. policies."3rdparty".Extensions = lib.mkIf ...) actually
  # resolve, instead of being stored as opaque definition wrappers.
  hmStub = { lib, ... }: {
    options.programs.firefox = {
      enable = lib.mkEnableOption "firefox";
      policies = lib.mkOption {
        type = lib.types.attrsOf lib.types.anything;
        default = { };
      };
    };
  };

  # Evaluate the module with a given set of `hearth.firefox.*` settings.
  evalFirefox = userSettings:
    (lib.evalModules {
      modules = [
        firefoxModule
        hmStub
        { config._module.args = { inherit pkgs; }; }
        { hearth.firefox = { enable = true; } // userSettings; }
      ];
    }).config.programs.firefox;

  # ---- Test fixtures + assertions ----

  # Fixture 1: bare-minimum config (just consoleUrl). Asserts on defaults.
  bare = evalFirefox { consoleUrl = "https://console.example.com"; };

  # Fixture 2: full setup — Nextcloud + Vaultwarden + role + per-role bookmarks.
  full = evalFirefox {
    consoleUrl = "https://hearth.example.com";
    nextcloudUrl = "https://cloud.example.com";
    vaultwardenUrl = "https://vault.example.com";
    services = [
      { name = "Chat"; url = "https://chat.example.com"; }
      { name = "Cloud"; url = "https://cloud.example.com"; }
    ];
    extraBookmarks = [{ name = "Wiki"; url = "https://wiki.example.com"; }];
    role = "admin";
    roleBookmarks = {
      admin = [{ name = "Grafana"; url = "https://grafana.example.com"; }];
      developer = [{ name = "CI"; url = "https://ci.example.com"; }];
    };
    dnsOverHttps.enable = true;
    dnsOverHttps.providerUrl = "https://dns.example.com/dns-query";
  };

  # Fixture 3: nextcloudUrl unset → Floccus extension must NOT appear.
  noNextcloud = evalFirefox {
    consoleUrl = "https://hearth.example.com";
    nextcloudUrl = null;
    vaultwardenUrl = null;
  };

  # Find a bookmark by name in the ManagedBookmarks list.
  findBookmark = name: bookmarks:
    lib.findFirst (b: (b.name or null) == name) null bookmarks;

  # Assertions — each `assert msg cond` aborts the eval if cond is false.
  checks =
    let
      barePolicies = bare.policies;
      fullPolicies = full.policies;
      noNcPolicies = noNextcloud.policies;
    in
    # --- Bare config ---
    assert lib.assertMsg (bare.enable == true)
      "enable should propagate to programs.firefox.enable";
    assert lib.assertMsg (barePolicies.DisableTelemetry == true)
      "telemetry must be disabled by default";
    assert lib.assertMsg (barePolicies.DisablePocket == true)
      "Pocket must be disabled by default";
    assert lib.assertMsg (barePolicies.Homepage.URL == "https://console.example.com")
      "Homepage.URL must equal consoleUrl";
    assert lib.assertMsg (barePolicies.ExtensionSettings ? "uBlock0@raymondhill.net")
      "uBlock Origin must always be force-installed";
    assert lib.assertMsg (barePolicies.ExtensionSettings."uBlock0@raymondhill.net".installation_mode == "force_installed")
      "uBlock Origin must use force_installed mode";

    # --- Full config: nextcloud + vaultwarden + role bookmarks ---
    assert lib.assertMsg (fullPolicies.ExtensionSettings ? "floccus@niclas-arndt.de")
      "Floccus must be installed when nextcloudUrl is set";
    assert lib.assertMsg (fullPolicies.ExtensionSettings ? "{446900e4-71c2-419f-a6a7-df9c091e268b}")
      "Bitwarden must be installed when vaultwardenUrl is set";
    assert lib.assertMsg (
      fullPolicies."3rdparty".Extensions."floccus@niclas-arndt.de".managedAccounts ==
      [{ type = "nextcloud-bookmarks"; url = "https://cloud.example.com"; label = "Hearth Bookmarks"; }]
    ) "Floccus 3rdparty config must point at the configured Nextcloud URL";
    assert lib.assertMsg (
      fullPolicies."3rdparty".Extensions."{446900e4-71c2-419f-a6a7-df9c091e268b}".environment.base ==
      "https://vault.example.com"
    ) "Bitwarden 3rdparty config must point at the configured Vaultwarden URL";

    # ManagedBookmarks: [{toplevel_name=Hearth}, services..., extraBookmarks..., roleBookmarks...]
    assert lib.assertMsg
      ((builtins.head fullPolicies.ManagedBookmarks).toplevel_name or null == "Hearth")
      "ManagedBookmarks must start with the Hearth toplevel folder";
    assert lib.assertMsg (findBookmark "Chat" fullPolicies.ManagedBookmarks != null)
      "Service bookmarks must appear in ManagedBookmarks";
    assert lib.assertMsg (findBookmark "Wiki" fullPolicies.ManagedBookmarks != null)
      "extraBookmarks must appear in ManagedBookmarks";
    assert lib.assertMsg (findBookmark "Grafana" fullPolicies.ManagedBookmarks != null)
      "role=admin must add the admin-specific bookmark (Grafana)";
    assert lib.assertMsg (findBookmark "CI" fullPolicies.ManagedBookmarks == null)
      "developer-role bookmarks must NOT appear when role=admin";

    # DNS-over-HTTPS
    assert lib.assertMsg (fullPolicies.DNSOverHTTPS.Enabled == true)
      "DNSOverHTTPS must be on when dnsOverHttps.enable is true";
    assert lib.assertMsg (fullPolicies.DNSOverHTTPS.ProviderURL == "https://dns.example.com/dns-query")
      "DNSOverHTTPS must carry the configured provider URL";

    # --- noNextcloud: opt-out path ---
    assert lib.assertMsg (! (noNcPolicies.ExtensionSettings ? "floccus@niclas-arndt.de"))
      "Floccus must NOT appear when nextcloudUrl is null";
    assert lib.assertMsg (! (noNcPolicies.ExtensionSettings ? "{446900e4-71c2-419f-a6a7-df9c091e268b}"))
      "Bitwarden must NOT appear when vaultwardenUrl is null";

    "ok";

in
pkgs.runCommand "hearth-firefox-module-eval" { } ''
  # Force evaluation of `checks` — if any assertion failed, this won't reach
  # the touch below.
  echo "${checks}" > /dev/null
  touch $out
''
