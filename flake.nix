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
  };

  outputs = { self, nixpkgs, crane, rust-overlay, flake-utils, ... }:
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

        lib = pkgs.lib;

        # VM integration tests (Linux only)
        vmTests = lib.optionalAttrs pkgs.stdenv.isLinux {
          vm-agent-polling = import ./tests/agent-polling.nix { inherit pkgs lib; };
          vm-desktop-baseline = import ./tests/desktop-baseline.nix { inherit pkgs lib; };
        };

        # Enrollment ISO image (Linux only)
        enrollmentImage = lib.optionalAttrs pkgs.stdenv.isLinux {
          enrollment-iso = (import ./lib/mk-enrollment-image.nix {
            inherit self nixpkgs system;
          }).config.system.build.isoImage;
        };

      in {
        checks = {
          inherit hearth-common hearth-agent hearth-greeter hearth-enrollment hearth-api;
          inherit workspaceClippy workspaceFmt workspaceTests;
        } // vmTests;

        packages = {
          inherit hearth-common hearth-agent hearth-greeter hearth-enrollment hearth-api;
          default = hearth-agent;
        } // enrollmentImage;

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

            # Utilities
            jq
            httpie
          ];

          # Environment variables for development
          DATABASE_URL = "postgres://hearth:hearth@localhost:5432/hearth";
          SQLX_OFFLINE = "true";
          RUST_LOG = "info";
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
