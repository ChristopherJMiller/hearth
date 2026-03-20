{
  description = "Hearth — Enterprise NixOS desktop fleet management platform";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    crane.url = "github:ipetkov/crane";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, rust-overlay, flake-utils, home-manager, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Use the stable toolchain specified in rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common source filtering — only include Rust/TOML/SQL/HTML files for builds
        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*\.sql$" path != null)
            || (builtins.match ".*\.toml$" path != null)
            || (builtins.match ".*\.html$" path != null)
            || (builtins.match ".*\.css$" path != null)
            || (builtins.match ".*\.svg$" path != null);
        };

        # Common build arguments shared across all builds
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = with pkgs; [
            openssl
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            gtk4
            glib
            gdk-pixbuf
            pango
            cairo
            graphene
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            mold
            clang
          ];
        };

        # Build workspace dependencies first (cached layer)
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Individual crate builds
        hearth-common = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-common";
        });

        hearth-agent = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-agent";
        });

        hearth-greeter = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-greeter";
        });

        hearth-enrollment = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-enrollment";
        });

        hearth-api = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-api";
        });

        hearth-build-worker = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-p hearth-build-worker";
        });

        # OCI container images (Linux only)
        ociImages = lib.optionalAttrs pkgs.stdenv.isLinux {
          hearth-api-image = pkgs.dockerTools.buildLayeredImage {
            name = "hearth-api";
            tag = "latest";
            contents = [ hearth-api pkgs.cacert ];
            config = {
              Cmd = [ "${hearth-api}/bin/hearth-api" ];
              Env = [
                "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
              ];
              ExposedPorts = { "3000/tcp" = {}; };
            };
          };

          hearth-build-worker-image = pkgs.dockerTools.buildLayeredImage {
            name = "hearth-build-worker";
            tag = "latest";
            contents = [
              hearth-build-worker
              pkgs.nix
              pkgs.nix-eval-jobs
              pkgs.attic-client
              pkgs.cacert
              pkgs.coreutils
              pkgs.bash
              pkgs.gitMinimal
            ];
            config = {
              Cmd = [ "${hearth-build-worker}/bin/hearth-build-worker" ];
              Env = [
                "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
                "NIX_CONFIG=experimental-features = nix-command flakes"
              ];
            };
          };
        };

        # Workspace-wide checks
        workspaceClippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--workspace -- --deny warnings";
        });

        workspaceFmt = craneLib.cargoFmt {
          inherit src;
        };

        workspaceTests = craneLib.cargoNextest (commonArgs // {
          inherit cargoArtifacts;
          cargoNextestExtraArgs = "--workspace";
        });

        # Helm chart validation (lint + kubeconform)
        helmChartLint = pkgs.runCommand "helm-chart-lint" {
          nativeBuildInputs = [ pkgs.kubernetes-helm pkgs.kubeconform ];
        } ''
          # helm lint
          helm lint ${./chart/hearth-home} --strict

          # Render and validate with kubeconform (skip CRDs like ServiceMonitor)
          helm template hearth ${./chart/hearth-home} \
            --set capabilities.observability=false \
            | kubeconform -strict -ignore-missing-schemas -kubernetes-version 1.29.0

          # Render with all capabilities (except observability which pulls subcharts)
          helm template hearth ${./chart/hearth-home} \
            --set capabilities.identity=true \
            --set capabilities.mesh=true \
            --set capabilities.builds=true \
            --set capabilities.observability=false \
            | kubeconform -strict -ignore-missing-schemas -kubernetes-version 1.29.0

          mkdir -p $out
          echo "Helm chart lint + kubeconform passed" > $out/result
        '';

        lib = pkgs.lib;

        # pkgs with kanidm allowed (marked insecure in this nixpkgs)
        kanidmPkgs = import nixpkgs {
          inherit system;
          config.permittedInsecurePackages = [ "kanidm-1.7.4" ];
        };

        # VM integration tests (Linux only)
        vmTests = lib.optionalAttrs pkgs.stdenv.isLinux {
          vm-agent-polling = import ./tests/agent-polling.nix { inherit pkgs lib; };
          vm-desktop-baseline = import ./tests/desktop-baseline.nix { inherit pkgs lib; };
          vm-full-enrollment = import ./tests/full-enrollment.nix { inherit pkgs lib hearth-enrollment hearth-agent; };
          vm-agent-heartbeat = import ./tests/agent-heartbeat.nix { inherit pkgs lib hearth-agent; };
          vm-offline-fallback = import ./tests/offline-fallback.nix { inherit pkgs lib hearth-agent; };
          vm-agent-system-switch = import ./tests/agent-system-switch.nix { inherit pkgs lib hearth-agent; };
          vm-kanidm-auth = import ./tests/kanidm-auth.nix { pkgs = kanidmPkgs; inherit lib; };
          vm-headscale-mesh = import ./tests/headscale-mesh.nix { inherit pkgs lib hearth-agent; };
          vm-agent-config-roundtrip = import ./tests/agent-config-roundtrip.nix { inherit pkgs lib hearth-agent; };
          vm-full-login-flow = import ./tests/full-login-flow.nix {
            pkgs = kanidmPkgs;
            inherit lib hearth-agent hearth-greeter;
          };
        };

        # Enrollment ISO image (Linux only)
        enrollmentImage = lib.optionalAttrs pkgs.stdenv.isLinux {
          enrollment-iso = (import ./lib/mk-enrollment-image.nix {
            inherit self nixpkgs system;
            serverUrl = "http://api.hearth.local:3000";
            cacheUrl = "http://cache.hearth.local:8080/hearth";
            kanidmUrl = "https://kanidm.hearth.local:8443";
            kanidmCaCert = ./dev/kanidm/cert.pem;
          }).config.system.build.image;
        };

        # Fleet dev VM (Linux only)
        fleetVm = lib.optionalAttrs pkgs.stdenv.isLinux {
          fleet-vm = (import ./dev/fleet-vm.nix {
            inherit self nixpkgs system;
          }).config.system.build.vm;
        };

      in {
        checks = {
          inherit hearth-common hearth-agent hearth-greeter hearth-enrollment hearth-api hearth-build-worker;
          inherit workspaceClippy workspaceFmt workspaceTests;
          inherit helmChartLint;
        } // vmTests;

        packages = {
          inherit hearth-common hearth-agent hearth-greeter hearth-enrollment hearth-api hearth-build-worker;
          default = hearth-agent;
        } // enrollmentImage // fleetVm // ociImages;

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            # Rust toolchain (provided by craneLib.devShell via checks)

            # Database
            sqlx-cli
            postgresql_16

            # Containers
            docker-compose

            # Nix tooling
            nix-eval-jobs
            attic-client

            # GTK4 dev
            gtk4
            glib
            gdk-pixbuf
            pango
            cairo
            graphene
            gobject-introspection

            # Linker
            mold
            clang

            # Build tools
            pkg-config
            openssl

            # Testing
            cargo-nextest
            cargo-watch

            # Frontend (Vite + React)
            nodejs_22
            pnpm
            nodePackages.typescript

            # Identity management (kanidm 1.9 needs Rust 1.93, built via rust-overlay)
            (pkgs.callPackage ./nix/kanidm-cli.nix {
              inherit (pkgs) rust-bin;
            })

            # Helm chart tooling
            kubernetes-helm
            chart-testing
            kubeconform
            kind

            # Utilities
            jq
            httpie
            just
          ];

          # Environment variables for development
          DATABASE_URL = "postgres://hearth:hearth@localhost:5432/hearth";
          SQLX_OFFLINE = "true";
          RUST_LOG = "info";
          HEARTH_ATTIC_CACHE = "hearth";
          HEARTH_ATTIC_SERVER = "http://localhost:8080";
          # Shared HS256 secret for minting Attic cache tokens.
          # Must match token-hs256-secret-base64 in dev/attic/server.toml.
          # Production: inject from secrets manager, rotate in lockstep with Attic.
          HEARTH_ATTIC_TOKEN_SECRET = "aGVhcnRoLWRldi1zZWNyZXQtZG8tbm90LXVzZS1pbi1wcm9kdWN0aW9uISEhIQ==";
        };
      }
    ) // {
      # ===== System-independent outputs =====

      # --- Overlay: adds Hearth packages to any nixpkgs ---
      overlays.default = final: prev: {
        hearth-agent = self.packages.${prev.system}.hearth-agent;
        hearth-greeter = self.packages.${prev.system}.hearth-greeter;
        hearth-enrollment = self.packages.${prev.system}.hearth-enrollment;
        hearth-api = self.packages.${prev.system}.hearth-api;
        hearth-build-worker = self.packages.${prev.system}.hearth-build-worker;
      };

      # --- NixOS Modules ---
      nixosModules = {
        agent = import ./modules/agent.nix;
        greeter = import ./modules/greeter.nix;
        pam = import ./modules/pam.nix;
        desktop = import ./modules/desktop.nix;
        hardening = import ./modules/hardening.nix;
        enrollment = import ./modules/enrollment.nix;
        roles = import ./modules/roles/default.nix;

        # Meta-module: imports everything a fleet machine needs
        hearth = {
          imports = [
            ./modules/agent.nix
            ./modules/greeter.nix
            ./modules/pam.nix
            ./modules/desktop.nix
            ./modules/hardening.nix
            ./modules/roles/default.nix
          ];
        };
      };

      # --- Home-Manager Modules (role profiles) ---
      homeModules = {
        common = import ./home-modules/common.nix;
        default = import ./home-modules/default.nix;
        developer = import ./home-modules/developer.nix;
        designer = import ./home-modules/designer.nix;
        admin = import ./home-modules/admin.nix;
      };

      # --- Fleet host builder helper ---
      lib.mkFleetHost = import ./lib/mk-fleet-host.nix {
        inherit self nixpkgs;
      };

      # --- Build-pipeline entry point: build a machine config from instance data JSON ---
      #
      # The build worker writes a per-machine instance-data JSON to disk, then
      # generates an eval.nix that calls this function. Signature:
      #
      #   buildMachineConfig { instanceDataPath = "/tmp/build-xxx/ws-0042.json"; }
      #
      # The JSON has the machine's hostname, role, machineId, tags, extra_config,
      # hardware_config (the raw Nix code from nixos-generate-config), etc.
      lib.buildMachineConfig = { instanceDataPath }:
        let
          data = builtins.fromJSON (builtins.readFile instanceDataPath);
          # If hardware_config is present (Nix source code string), write it to a
          # derivation so we can import it as a module.
          hardwareModule =
            if data ? hardware_config && data.hardware_config != null
            then builtins.toFile "hardware-configuration.nix" data.hardware_config
            else null;
        in self.lib.mkFleetHost ({
          hostname = data.hostname;
          role = data.role or "default";
          machineId = data.machine_id;
          serverUrl = data.server_url or "http://localhost:3000";
          hardware = if hardwareModule != null then import hardwareModule else null;
          extraConfig = {
            fileSystems."/" = { device = "/dev/disk/by-label/nixos"; fsType = "ext4"; };
            fileSystems."/boot" = { device = "/dev/disk/by-label/boot"; fsType = "vfat"; };
          } // (if data ? extra_config && data.extra_config != null then data.extra_config else {});
        } // nixpkgs.lib.optionalAttrs (data ? kanidm_url && data.kanidm_url != null) {
          kanidmUrl = data.kanidm_url;
        } // nixpkgs.lib.optionalAttrs (data ? binary_cache_url && data.binary_cache_url != null) {
          binaryCacheUrl = data.binary_cache_url;
        } // nixpkgs.lib.optionalAttrs (data ? tags && data.tags != null) {
          # Tags could drive extra modules in the future
        });

      # --- Build-pipeline entry point: build a per-user home-manager closure ---
      #
      # The build worker writes a per-user config JSON to disk, then generates
      # a user-eval.nix that calls this function. The JSON has the user's
      # username, base_role, and overrides (extra packages, git config, etc.).
      #
      #   buildUserEnv { userConfigPath = "/tmp/user-build-xxx/user-config.json"; }
      #
      lib.buildUserEnv = { userConfigPath }:
        let
          cfg = builtins.fromJSON (builtins.readFile userConfigPath);
          lib = nixpkgs.lib;
          pkgs = nixpkgs.legacyPackages.x86_64-linux;
          roleModule = self.homeModules.${cfg.base_role} or self.homeModules.default;

          # Build an override module from structured JSON fields.
          overrideModule = { config, lib, pkgs, ... }: {
            programs.git = lib.mkIf (cfg.overrides ? git) (
              lib.optionalAttrs (cfg.overrides.git ? user_name) {
                userName = cfg.overrides.git.user_name;
              } // lib.optionalAttrs (cfg.overrides.git ? user_email) {
                userEmail = cfg.overrides.git.user_email;
              }
            );

            # TODO: Add a configurable package allowlist/denylist. Currently any
            # nixpkgs attribute can be requested via extra_packages, which may
            # include security-sensitive tools on an enterprise fleet.
            home.packages = lib.optionals (cfg.overrides ? extra_packages)
              (map (name: pkgs.${name}) cfg.overrides.extra_packages);

            home.sessionVariables =
              (lib.optionalAttrs (cfg.overrides ? editor) {
                EDITOR = cfg.overrides.editor;
                VISUAL = cfg.overrides.editor;
              }) // (cfg.overrides.session_variables or {});

            programs.bash.shellAliases = cfg.overrides.shell_aliases or {};
          };
        in home-manager.lib.homeManagerConfiguration {
          inherit pkgs;
          modules = [
            roleModule
            overrideModule
            {
              home.username = cfg.username;
              home.homeDirectory = "/home/${cfg.username}";
            }
          ];
        };

      # --- Example fleet host (uncomment to test) ---
      # nixosConfigurations.example-workstation = self.lib.mkFleetHost {
      #   hostname = "ws-example";
      #   role = "developer";
      #   machineId = "00000000-0000-0000-0000-000000000000";
      #   serverUrl = "https://api.hearth.example.com";
      #   homeFlakeRef = "github:myorg/fleet-config";
      #   hardware = null;
      #   extraModules = [ ];
      # };
    };
}
