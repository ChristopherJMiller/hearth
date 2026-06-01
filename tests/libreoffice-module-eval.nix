# tests/libreoffice-module-eval.nix — Pure-Nix eval test for
# home-modules/libreoffice.nix
#
# Validates the rendered registrymodifications.xcu, remote-servers.xml,
# and office.toml content by evaluating the module in isolation. No VM,
# no home-manager activation — just lib.evalModules with stubs for the
# bits of the HM option surface our module touches.

{ pkgs, lib ? pkgs.lib, ... }:

let
  libreofficeModule = import ../home-modules/libreoffice.nix;

  # Minimal stubs for the home-manager surface used by the module.
  hmStub = { lib, ... }: {
    options = {
      home.homeDirectory = lib.mkOption {
        type = lib.types.str;
        default = "/home/testuser";
      };
      home.packages = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        default = [ ];
      };
      home.activation = lib.mkOption {
        type = lib.types.attrs;
        default = { };
      };
      xdg.configFile = lib.mkOption {
        type = lib.types.attrsOf (lib.types.submodule {
          options = {
            text = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; };
            source = lib.mkOption { type = lib.types.nullOr lib.types.path; default = null; };
          };
        });
        default = { };
      };
    };
  };

  # lib.hm.dag stubs (the module uses lib.hm.dag.entryAfter)
  libWithHm = lib.extend (final: prev: {
    hm = (prev.hm or { }) // {
      dag = {
        entryAfter = _deps: txt: txt;
      };
    };
  });

  evalLO = userSettings:
    (libWithHm.evalModules {
      modules = [
        libreofficeModule
        hmStub
        { config._module.args = { inherit pkgs; lib = libWithHm; }; }
        { hearth.libreoffice = { enable = true; } // userSettings; }
      ];
    }).config;

  # ---- Fixtures ----

  basic = evalLO {
    nextcloudUrl = "https://cloud.example.com";
  };

  veryHigh = evalLO {
    nextcloudUrl = "https://cloud.example.com";
    macroSecurity = "very-high";
  };

  customFonts = evalLO {
    nextcloudUrl = "https://cloud.example.com";
    defaultFonts = {
      sans = "Inter";
      serif = "Charter";
      mono = "Iosevka";
    };
  };

  # ---- Assertions ----

  registry = basic.xdg.configFile."libreoffice/4/user/registrymodifications.xcu".text;
  remoteSvr = basic.xdg.configFile."libreoffice/4/user/remote-servers.xml".text;
  officeToml = basic.xdg.configFile."hearth/office.toml".text;

  vhRegistry = veryHigh.xdg.configFile."libreoffice/4/user/registrymodifications.xcu".text;
  cfRegistry = customFonts.xdg.configFile."libreoffice/4/user/registrymodifications.xcu".text;

  contains = needle: haystack: lib.hasInfix needle haystack;

  checks =
    # --- Basic registry rendering ---
    assert lib.assertMsg (contains "<value>true</value>" registry)
      "registrymodifications.xcu must enable file locking";
    assert lib.assertMsg (contains "<value>false</value>" registry)
      "telemetry/crash-report must be disabled (rendered as <value>false</value>)";
    assert lib.assertMsg (contains "MacroSecurityLevel" registry)
      "MacroSecurityLevel path must be set";
    # Default macro security = "high" → level 3
    assert lib.assertMsg (contains "MacroSecurityLevel" registry && contains "<value>3</value>" registry)
      "default macroSecurity=high must render as level 3";

    # --- very-high overrides to level 4 ---
    assert lib.assertMsg (contains "<value>4</value>" vhRegistry)
      "macroSecurity=very-high must render as level 4";

    # --- Default fonts ---
    assert lib.assertMsg (contains "<value>DM Sans</value>" registry)
      "default sans font must be DM Sans";
    assert lib.assertMsg (contains "<value>Noto Serif</value>" registry)
      "default serif font must be Noto Serif";
    assert lib.assertMsg (contains "<value>JetBrains Mono</value>" registry)
      "default mono font must be JetBrains Mono";

    # --- Custom fonts override ---
    assert lib.assertMsg (contains "<value>Inter</value>" cfRegistry)
      "custom sans font must render";
    assert lib.assertMsg (! contains "<value>DM Sans</value>" cfRegistry)
      "custom-font fixture must NOT carry the default DM Sans";

    # --- Default save/template paths derive from homeDirectory ---
    assert lib.assertMsg (contains "/home/testuser/Nextcloud/Documents" registry)
      "default save dir must be ~/Nextcloud/Documents";
    assert lib.assertMsg (contains "/home/testuser/Nextcloud/Templates" registry)
      "default template dir must be ~/Nextcloud/Templates";

    # --- remote-servers.xml ---
    assert lib.assertMsg (contains "Hearth Cloud Storage" remoteSvr)
      "remote-servers.xml must label the entry 'Hearth Cloud Storage'";
    assert lib.assertMsg (contains "https://cloud.example.com/remote.php/dav/files/" remoteSvr)
      "remote-servers.xml URL must be the WebDAV path";
    assert lib.assertMsg (contains "<Type>WEBDAV</Type>" remoteSvr)
      "remote server type must be WEBDAV";

    # --- office.toml (consumed by hearth-office .oxt extension) ---
    assert lib.assertMsg (contains "url = \"https://cloud.example.com\"" officeToml)
      "office.toml must record the Nextcloud root URL";
    assert lib.assertMsg (contains "webdav_url = \"https://cloud.example.com/remote.php/dav/files/\"" officeToml)
      "office.toml must record the WebDAV URL";

    "ok";

in
pkgs.runCommand "hearth-libreoffice-module-eval" { } ''
  echo "${checks}" > /dev/null
  touch $out
''
