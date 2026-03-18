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
#     kanidmUrl = "https://idm.hearth.example.com";
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
, roleMapping ? [ ]
, defaultRole ? "default"
, hardeningLevel ? "standard"
, complianceProfile ? null
, enableDesktop ? true
, enableEnrollment ? false
, kanidmUrl ? null
, kanidmCaCert ? null
, hardwareProfile ? null
, secureBoot ? false
, tpmFde ? false
, tpmDevice ? "/dev/disk/by-partlabel/cryptroot"
, diskoConfig ? null
, diskoDevice ? "/dev/sda"
, metricsRemoteWriteUrl ? null
, lokiUrl ? null
, branding ? { }
, extraModules ? [ ]
, extraConfig ? { }
}:

let lib = nixpkgs.lib; in
nixpkgs.lib.nixosSystem {
  # Pass inputs as specialArgs so modules can access flake inputs
  specialArgs = {
    inherit inputs self;
  };

  modules = [
    # --- Hearth overlay: makes hearth-* packages available as pkgs.hearth-* ---
    {
      nixpkgs.hostPlatform = system;
      nixpkgs.overlays = [
        self.overlays.default
      ];
    }

    # --- Core Hearth modules ---
    ../modules/agent.nix
    ../modules/greeter.nix
    ../modules/pam.nix
    ../modules/kanidm-client.nix
    ../modules/desktop.nix
    ../modules/hardening.nix
    ../modules/compliance/default.nix
    ../modules/secure-boot.nix
    ../modules/tpm-fde.nix
    ../modules/logging.nix
    ../modules/metrics.nix
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
        inherit roleMapping defaultRole;
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
      services.hearth.pam = {
        enable = true;
        authBackend = if kanidmUrl != null then "kanidm" else "sssd";
      };

      # --- Kanidm client (when kanidmUrl is provided) ---
      services.hearth.kanidmClient = lib.mkIf (kanidmUrl != null) ({
        enable = true;
        uri = kanidmUrl;
      } // lib.optionalAttrs (kanidmCaCert != null) {
        caCertPath = kanidmCaCert;
      });

      # --- Desktop ---
      services.hearth.desktop.enable = enableDesktop;

      # --- Hardening ---
      services.hearth.hardening = {
        enable = true;
        level = hardeningLevel;
      };

      # --- Compliance ---
      services.hearth.compliance = lib.mkIf (complianceProfile != null) {
        profile = complianceProfile;
      };

      # --- Role ---
      services.hearth.roles.role = role;

      # --- Secure Boot ---
      services.hearth.secureBoot.enable = secureBoot;

      # --- TPM FDE ---
      services.hearth.tpmFde = lib.mkIf tpmFde { enable = true; device = tpmDevice; };

      # --- Logging ---
      services.hearth.logging = lib.mkIf (lokiUrl != null) { enable = true; inherit lokiUrl; };

      # --- Metrics ---
      services.hearth.metrics = lib.mkIf (metricsRemoteWriteUrl != null) { enable = true; remoteWriteUrl = metricsRemoteWriteUrl; };

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

    # --- Safety net: when no hardware config is provided, import not-detected.nix
    # which enables redistributable firmware and common initrd modules.
    # This prevents non-bootable systems when hardware-configuration.nix is missing. ---
  ] ++ lib.optional (hardware == null) ({ modulesPath, ... }: {
      imports = [ (modulesPath + "/installer/scan/not-detected.nix") ];
    })

    # --- Hardware configuration (if provided as a path to .nix file) ---
    ++ lib.optional (hardware != null) hardware

    # --- Hardware profile (if provided) ---
    ++ lib.optional (hardwareProfile != null) hardwareProfile

    # --- Disko configuration (if provided) ---
    ++ lib.optional (diskoConfig != null) (import diskoConfig { device = diskoDevice; })

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
