# lib/mk-fleet-host.nix — Parameterized builder for Hearth fleet machines
#
# This function takes instance parameters and produces a complete NixOS
# system configuration with all Hearth modules wired up. It is the primary
# entry point for fleet repositories defining managed machines.
#
# Usage from a fleet repository's flake.nix:
#
#   nixosConfigurations.ws-0042 = hearth.lib.mkFleetHost {
#     hostname = "ws-0042";
#     role = "developer";
#     machineId = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
#     serverUrl = "https://api.hearth.example.com";
#     hardware = ./hosts/ws-0042/hardware-configuration.nix;
#     extraModules = [ ./site-specific.nix ];
#   };
#

# First level: receives flake inputs from flake.nix
{ self, nixpkgs, ... }@inputs:

# Second level: receives per-host parameters from the fleet repo
{ hostname
, role ? "default"
, machineId
, serverUrl
, system ? "x86_64-linux"
, hardware ? null
, stateVersion ? "25.05"
, binaryCacheUrl ? null
, homeFlakeRef ? null
, roleMapping ? { default = "default"; }
, hardeningLevel ? "standard"
, enableDesktop ? true
, enableEnrollment ? false
, branding ? { }
, extraModules ? [ ]
, extraConfig ? { }
}:

nixpkgs.lib.nixosSystem {
  inherit system;

  # Pass inputs as specialArgs so modules can access flake inputs
  specialArgs = {
    inherit inputs self;
  };

  modules = [
    # --- Hearth overlay: makes hearth-* packages available as pkgs.hearth-* ---
    {
      nixpkgs.overlays = [
        self.overlays.default
      ];
    }

    # --- Core Hearth modules ---
    ../modules/agent.nix
    ../modules/greeter.nix
    ../modules/pam.nix
    ../modules/desktop.nix
    ../modules/hardening.nix
    ../modules/roles/default.nix

    # --- Per-host configuration ---
    ({ config, lib, pkgs, ... }: {
      # System identity
      networking.hostName = hostname;
      system.stateVersion = stateVersion;

      # --- Hearth agent ---
      services.hearth.agent = {
        enable = true;
        inherit serverUrl machineId;
        inherit roleMapping;
      } // lib.optionalAttrs (binaryCacheUrl != null) {
        inherit binaryCacheUrl;
      } // lib.optionalAttrs (homeFlakeRef != null) {
        inherit homeFlakeRef;
      };

      # --- Hearth greeter ---
      services.hearth.greeter = lib.mkIf enableDesktop ({
        enable = true;
      } // lib.optionalAttrs (branding ? organizationName) {
        branding.organizationName = branding.organizationName;
      } // lib.optionalAttrs (branding ? logo) {
        branding.logo = branding.logo;
      } // lib.optionalAttrs (branding ? cssOverride) {
        branding.cssOverride = branding.cssOverride;
      });

      # --- PAM/NSS ---
      services.hearth.pam.enable = true;

      # --- Desktop ---
      services.hearth.desktop.enable = enableDesktop;

      # --- Hardening ---
      services.hearth.hardening = {
        enable = true;
        level = hardeningLevel;
      };

      # --- Role ---
      services.hearth.roles.role = role;

      # --- Boot loader (reasonable defaults, hardware module can override) ---
      boot.loader.systemd-boot.enable = lib.mkDefault true;
      boot.loader.efi.canTouchEfiVariables = lib.mkDefault true;

      # --- Nix configuration ---
      nix = {
        settings = {
          experimental-features = [ "nix-command" "flakes" ];
          auto-optimise-store = true;
        };
        gc = {
          automatic = true;
          dates = "weekly";
          options = "--delete-older-than 30d";
        };
      };

      # --- Timezone and locale (fleet operators can override) ---
      time.timeZone = lib.mkDefault "UTC";
      i18n.defaultLocale = lib.mkDefault "en_US.UTF-8";
    })

    # --- Hardware configuration (if provided) ---
  ] ++ lib.optional (hardware != null) hardware

    # --- Enrollment module (if this is an enrollment image) ---
    ++ lib.optional enableEnrollment (
      { ... }: {
        imports = [ ../modules/enrollment.nix ];
        services.hearth.enrollment = {
          enable = true;
          inherit serverUrl;
        };
      }
    )

    # --- Extra modules from the fleet repo ---
    ++ extraModules

    # --- Inline extra config ---
    ++ lib.optional (extraConfig != { }) ({ ... }: extraConfig);
}
