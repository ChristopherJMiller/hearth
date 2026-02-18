# Hearth: Custom Components, Onboarding Flow, and Distribution Architecture

**Hearth** is the enterprise NixOS desktop management platform. This document details the custom software components that must be built, how the user onboarding and login flow works end-to-end, how these components integrate with existing NixOS primitives, and how everything is packaged, distributed, and updated through an internal package feed.

---

## Why GNOME, not KDE

The platform standardizes on GNOME as the desktop environment. This is a technical decision driven by three factors.

**Declarative settings management.** GNOME uses dconf, a binary database with a layered override system. System defaults live in `/etc/dconf/db/`, user customizations layer on top, and home-manager's `dconf.settings` maps cleanly to this model. KDE Plasma stores settings as mutable INI-style files scattered across `~/.config` â€” `kdeglobals`, `kwinrc`, `plasma-org.kde.plasma.desktop-appletsrc`, and dozens more. When a user opens KDE System Settings, it writes directly to those files. `plasma-manager` (the home-manager KDE module) exists but is fighting the architecture â€” it generates config files that Plasma overwrites at runtime. Declarative settings management works in GNOME in a way it fundamentally does not in KDE on NixOS today.

**Greeter integration.** The custom greeter (`hearth-greeter`) is built with `gtk4-rs`, maintained by the GNOME Rust team and production-grade. The Qt Rust bindings (`cxx-qt`, `qmetaobject-rs`) are less mature. A GTK4 greeter visually matches a GNOME desktop naturally.

**NixOS module maturity.** The NixOS GNOME module is one of the most used and most maintained DE modules in nixpkgs. The KDE/Plasma module has improved but remains rougher for declarative settings and extension handling.

---

## The login and onboarding flow

The onboarding experience is the architectural keystone. It must feel seamless to the user sitting at a login screen while solving the home-manager issue #5244 problem behind the scenes. The flow is built around **greetd** â€” a minimal login daemon with a simple IPC protocol â€” and a custom greeter that owns the entire authentication-through-desktop-launch pipeline.

### Why greetd, not GDM with background activation

The alternative to greetd is using GDM as the display manager and running home-manager activation as a background process after the desktop session starts. This approach has a fundamental problem: GNOME Shell extensions require a full shell restart to load (on Wayland, that means logout/login). GTK and icon themes partially apply live but cursor themes do not. Environment variables (`$EDITOR`, `$PATH` additions, proxy settings) are read once at session start. Systemd user services defined by home-manager need `daemon-reload` and explicit restarts. Autostart `.desktop` entries are only scanned at session start.

The background approach almost guarantees that first-time users need a logout/login cycle to get their full environment. Showing a notification that says "Your environment is ready, please log out and back in" is a terrible first impression.

greetd solves this by giving us a window between successful authentication and session launch. The user authenticates, we prepare their complete environment, and only then does the desktop session start. When GNOME appears, every dconf key is set, every symlink is in place, every environment variable is correct, all extensions are installed, all autostart entries exist.

### The greetd IPC protocol

greetd communicates with its greeter over a Unix socket using a simple JSON protocol:

1. Greeter sends `CreateSession { username: "jdoe" }` â€” greetd opens a PAM handle
2. greetd responds with `AuthMessage { auth_message_type: "secret", auth_message: "Password:" }`
3. Greeter collects the password, sends `PostAuthMessageResponse { response: "..." }`
4. PAM authenticates against SSSD/Kanidm â€” greetd sends `Success`
5. **The preparation window opens.** The user is authenticated but no desktop session exists yet.
6. Greeter transitions to "Preparing your workspace..." with a progress indicator
7. Greeter asks `hearth-agent` (over local Unix socket) to prepare the environment for this user
8. Agent creates the home directory, resolves and activates the home-manager closure
9. Agent reports completion â€” greeter sends `StartSession { cmd: ["gnome-session"], env: [...] }` to greetd
10. greetd opens the PAM session, sets up cgroups, and execs into the desktop

### Latency profile by scenario

**Returning user** (closure cached locally): Authentication takes 1â€“2 seconds (SSSD + Kerberos round trip), activation takes under 1 second (symlinking only). Total login time: 2â€“3 seconds. Indistinguishable from a normal GDM login.

**First-time user on this device, role profile pre-built**: Authentication 1â€“2 seconds, role profile activation 1â€“3 seconds (already in the local Nix store as part of the system closure). Total: 3â€“5 seconds. The "preparing" screen barely flashes.

**First-time user, per-user closure needs binary cache pull**: Authentication 1â€“2 seconds, binary cache pull 15â€“60 seconds depending on network and closure delta. The progress screen shows meaningful status. This scenario is mitigable by pre-warming: when a machine enrolls and gets assigned to the engineering team, the control plane pre-builds closures for every engineer and pushes them to the machine's local store overnight.

**Truly cold start** (no closure built yet): 1â€“3 minutes while the control plane builds. The progress screen shows "Building your environment..." and the user waits. This only happens once per user across the entire fleet, and is comparable to Windows "Setting up your account..." during first Intune enrollment.

### PAM stack coordination

When the greeter sends `StartSession`, greetd runs PAM `session_open` hooks. The PAM stack must be configured so that it handles authentication and basic session setup but does NOT attempt home-manager activation â€” that is owned entirely by the greeter-to-agent flow.

```nix
# modules/hearth-pam.nix
{ config, lib, pkgs, ... }:
{
  services.greetd = {
    enable = true;
    settings.default_session = {
      command = "${pkgs.hearth-greeter}/bin/hearth-greeter";
      user = "greeter";
    };
  };

  # PAM: auth via SSSD, home dir creation as backup, network mounts
  # No home-manager activation here â€” the greeter handles that
  security.pam.services.greetd = {
    makeHomeDir = true;
    pamMount = true;
    sssdStrictAccess = false;
  };

  # Disable GDM
  services.xserver.displayManager.gdm.enable = false;
}
```

`pam_mkhomedir` in the PAM session stack is kept as a safety net â€” if the agent already created the home directory, `pam_mkhomedir` sees it exists and is a no-op. If something fails in the agent path, PAM still creates a bare home directory so the session can start.

---

## Custom components to build

The greetd approach simplifies the component count significantly. By owning the login flow, the greeter eliminates the need for separate PAM hooks, first-login desktop notification apps, and standalone user environment services. Two primary binaries plus supporting NixOS modules cover the full on-device story.

### hearth-greeter

The custom greetd greeter. A Rust application using `gtk4-rs` for the UI and `greetd_ipc` for the login daemon protocol.

**Responsibilities:**

- Presents the branded login screen (username/password entry, organization logo, machine hostname)
- Drives the greetd authentication protocol (CreateSession â†’ AuthMessage â†’ PostAuthMessageResponse â†’ Success/Error)
- On successful authentication, transitions to a "Preparing your workspace..." view with progress
- Communicates with `hearth-agent` over a local Unix socket to trigger environment preparation
- Receives a stream of progress events from the agent and updates the UI accordingly
- On environment readiness, sends `StartSession` to greetd with the correct session command and environment variables
- Handles error states: authentication failure (retry), agent failure (offer to launch with default environment), network issues (offline fallback)

**UI states:**

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”    auth     â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”  success   â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚  Login       â”‚â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚  Authenticating  â”‚â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚  Preparing     â”‚
â”‚  Screen      â”‚â—„â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”‚  (spinner)       â”‚  failure  â”‚  Environment   â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜    retry    â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜            â””â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                                                                    â”‚
                                                             agent Ready
                                                                    â”‚
                                                            â”Œâ”€â”€â”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”€â”€â”€â”
                                                            â”‚  StartSession  â”‚
                                                            â”‚  â†’ GNOME       â”‚
                                                            â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

**UI toolkit rationale:** `gtk4-rs` is the right choice because the greeter runs on a GNOME desktop. It supports Wayland natively, handles HiDPI scaling, and can be themed with CSS to match organizational branding. The GNOME Rust team actively maintains the bindings.

### hearth-agent

The primary on-device daemon. Runs as a systemd system service under root. Handles all communication with the Hearth control plane and manages the device's lifecycle.

**Responsibilities:**

- Maintains a persistent connection to the control plane over the Headscale mesh (HTTPS or gRPC)
- Sends periodic heartbeats: current system generation, running closure path, kernel version, disk/memory usage, connectivity status
- Receives push notifications: pending deployments, user environment updates, remote action requests
- Exposes a local Unix socket API that `hearth-greeter` uses to request user environment preparation
- Manages user environment activation: resolves closures, pulls from binary cache, runs activation scripts
- Writes Prometheus textfile metrics to `/var/lib/prometheus-node-exporter/hearth.prom`
- Reports user login events to the control plane for per-user closure generation
- Handles pre-warming: accepts and caches user closures pushed by the control plane ahead of time

**User environment preparation flow** (triggered by greeter via Unix socket):

1. **Create home directory** if it doesn't exist â€” `mkdir`, `chown`, create XDG base directories. This replaces `pam_mkhomedir` as the primary mechanism.

2. **Resolve the user's closure** â€” Check the local state database (`/var/lib/hearth/user-envs.db`, SQLite):
   - Per-user closure cached locally? â†’ Use it (fastest path)
   - Role profile for the user's resolved role cached? â†’ Use it
   - Neither? â†’ Fall back to the pre-built role profile embedded in the system closure

3. **Ensure the closure is in the local Nix store** â€” If the resolved closure path isn't in `/nix/store`, pull it from the binary cache via `nix copy --from`. Parse progress output to report to the greeter.

4. **Activate the closure** â€” Run the home-manager activation script as the target user:
   ```
   Command::new(activation_script)
       .uid(uid).gid(gid)
       .env("HOME", home_dir)
       .env("USER", username)
       .env("XDG_CONFIG_HOME", home_dir.join(".config"))
       .env("XDG_DATA_HOME", home_dir.join(".local/share"))
   ```

5. **Report login to control plane** (async, does not block the user) â€” If this is a new user-device association, the control plane starts building a per-user closure for next time.

6. **Send `Ready` event** to the greeter over the Unix socket.

**The IPC protocol between greeter and agent:**

```rust
// hearth-common/src/ipc.rs

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentRequest {
    PrepareUserEnv {
        username: String,
        uid: u32,
        gid: u32,
        home: PathBuf,
        groups: Vec<String>,
    },
    /// Query agent status (greeter startup health check)
    Ping,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// Home directory created or already exists
    HomeReady,
    /// Cached closure found locally, activating now
    ActivatingCached { closure_path: String },
    /// Pulling closure from binary cache
    Pulling { closure_path: String, progress_pct: Option<f32> },
    /// No per-user or role closure available, using system-embedded role profile
    FallingBackToRoleProfile { role: String },
    /// Activation complete â€” environment is ready for session launch
    Ready,
    /// Control plane is building a per-user closure (truly first login anywhere)
    BuildingRemote { estimated_seconds: Option<u32> },
    /// Something went wrong
    Error { message: String, recoverable: bool },
    /// Response to Ping
    Pong { version: String, connected_to_control_plane: bool },
}
```

The greeter subscribes to a stream of `AgentEvent` messages and updates the progress UI with contextual messages: "Creating your workspace..." (HomeReady), "Loading your developer environment..." (ActivatingCached or Pulling), "Building your personalized environment â€” this may take a couple of minutes..." (BuildingRemote).

### hearth-enrollment

A TUI application for device enrollment. Runs in the enrollment netboot image or USB image. Built with `ratatui` for the terminal interface.

**Responsibilities:**

- Runs hardware detection via NixOS Facter (CPU, GPU, disk topology, network interfaces, TPM presence, Secure Boot state, serial number, MAC addresses)
- Generates a hardware fingerprint (hash of serial + TPM EK certificate + motherboard UUID)
- Provides network setup (wired auto-detection, WiFi SSID selection and password entry)
- Contacts the control plane enrollment endpoint
- Displays the enrollment code for token-based enrollment (the "enter the code shown on screen" pattern)
- Shows provisioning progress: disk partitioning, closure download, NixOS installation, Secure Boot enrollment, TPM enrollment
- Reboots into the installed system on completion

### hearth-common

A shared Rust library crate used by all three binaries. Contains:

- IPC protocol types (`AgentRequest`, `AgentEvent`) â€” ensures type-safe communication between greeter and agent at compile time
- Control plane API client (REST, using `reqwest`)
- Configuration file parsing (`/etc/hearth/agent.toml`, `/etc/hearth/greeter.toml`)
- Nix store path utilities â€” wrapping `nix-compat` from the tvix project for store path parsing without shelling out, while using `nix` CLI for network operations like `nix copy`

### Components eliminated by the greetd approach

The following components from earlier architecture drafts are no longer needed:

- **PAM session hook binary** â€” The greeter handles activation before the PAM session opens. No `pam_exec.so` hook required.
- **First-login notification app** â€” The greeter IS the progress UI. No desktop autostart application needed.
- **Separate user environment service** â€” Folded into `hearth-agent`. The agent runs as root and handles activation directly, including running the activation script as the target user.
- **Session wrapper script** â€” The greeter sends `StartSession` with the correct command. No wrapper needed.

---

## The role profile fallback system

Pre-built role profiles provide instant activation even when no per-user closure exists. These are home-manager configurations built at NixOS system build time â€” one per organizational role â€” and embedded in the system closure.

### How role profiles are built

In the Hearth NixOS module, each role maps to a home-manager configuration module:

```nix
# modules/hearth-roles.nix
{ config, lib, pkgs, ... }:
let
  mkRoleProfile = role: hmConfig:
    (import <home-manager/modules> {
      inherit pkgs;
      configuration = { ... }: {
        imports = [ hmConfig ];
        # Template values â€” the agent rewrites these at activation time
        home.username = "hearth-template";
        home.homeDirectory = "/home/hearth-template";
        home.stateVersion = "25.05";
      };
    }).activationPackage;
in
{
  options.services.hearth.roles = lib.mkOption {
    type = lib.types.attrsOf lib.types.path;
    description = "Map of role names to home-manager module paths";
  };

  config = {
    # Pre-build all role profiles and store them in the system closure
    environment.etc = lib.mapAttrs' (role: _: {
      name = "hearth/role-profiles/${role}";
      value.source = mkRoleProfile role config.services.hearth.roles.${role};
    }) config.services.hearth.roles;
  };
}
```

Fleet configuration defines the roles:

```nix
services.hearth.roles = {
  developer = ./home-modules/developer.nix;
  designer = ./home-modules/designer.nix;
  admin = ./home-modules/admin.nix;
  default = ./home-modules/default.nix;
};
```

### Username parameterization at activation time

The pre-built role profiles contain a template username (`hearth-template`) hardcoded into the activation script and symlink targets. When the agent activates a role profile for a real user, it must rewrite these paths.

The agent handles this with a dedicated activation function (`hearth-agent/src/activate.rs`) that:

1. Reads the activation script from the role profile store path
2. Performs string replacement of `/home/hearth-template` â†’ `/home/jdoe` and `hearth-template` â†’ `jdoe`
3. Writes the rewritten script to a temporary location
4. Executes it as the target user

This is acknowledged as fragile â€” it breaks if home-manager changes its activation script format. The mitigation is pinning the home-manager version in the Hearth flake and testing activation as part of CI. The per-user closure path (generated on the control plane with the real username) is always preferred when available. Role profiles are a fast fallback, not the steady state.

### Group-to-role resolution

When a user logs in, the agent receives their group list from the greeter (which got it from SSSD during PAM authentication). The agent resolves groups to a role using a priority-ordered mapping defined in the Hearth configuration:

```toml
# /etc/hearth/agent.toml
[role-mapping]
# First match wins. Groups are checked in order.
devops = "developer"
engineering = "developer"
design = "designer"
it-admin = "admin"
# Fallback for users matching no group
default = "default"
```

---

## Offline operation

The agent must handle offline scenarios gracefully. A laptop taken on a plane, a device on a flaky hotel network, a new office with delayed internet provisioning.

**User has logged in on this device before:** Their home-manager closure is in the local Nix store. Activation works entirely offline â€” just symlinking, under one second. SSSD credential caching handles offline authentication against the identity provider.

**User has never logged in on this device and it's offline:** The agent falls back to the pre-built role profile embedded in the system closure. The user gets a fully functional environment matching their role (determined by cached SSSD group data), just without per-user customizations. When connectivity returns, the agent syncs with the control plane, pulls the per-user closure, and activates it. The next login is seamless.

**Device cannot reach the control plane at all:** comin (the GitOps agent for system-level config) and the Hearth agent both operate in degraded mode. The device continues running its last known-good configuration. All locally cached user closures continue to work. The agent queues login events and heartbeats for transmission when connectivity restores.

---

## Interacting with the Nix store

The agent needs to interact with the Nix store for three operations: checking if a store path exists locally, pulling closures from the binary cache, and running activation scripts.

**Recommended approach: shell out to the `nix` CLI for network operations, use `nix-compat` for local queries.**

The `nix-compat` crate from the tvix project can parse store paths, compute derivation hashes, and read NAR archives in pure Rust. Use it for fast local checks like "is `/nix/store/abc123-home-manager-generation` present?" (a stat call), and "parse this closure's references." Shell out to `nix copy --from https://cache.example.com /nix/store/abc123...` for binary cache pulls â€” this is what every other Nix tool does, it handles all the HTTP/compression/signing logic, and its output can be parsed for progress reporting.

Starting with full CLI shelling and only pulling in `nix-compat` where performance matters is the right incremental approach.

---

## The internal package feed

Fleet machines must pull all custom Hearth components from a controlled, versioned source. This is structured as a Nix flake with an overlay and NixOS modules, distributed through an Attic binary cache.

### Repository structure

```
hearth/
â”œâ”€â”€ Cargo.toml                  # Rust workspace root
â”œâ”€â”€ Cargo.lock
â”œâ”€â”€ crates/
â”‚   â”œâ”€â”€ hearth-greeter/
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src/
â”‚   â”‚       â”œâ”€â”€ main.rs         # Entry point, GTK app setup
â”‚   â”‚       â”œâ”€â”€ greeter.rs      # greetd IPC protocol handling
â”‚   â”‚       â”œâ”€â”€ agent_client.rs # Unix socket client for hearth-agent
â”‚   â”‚       â”œâ”€â”€ ui/
â”‚   â”‚       â”‚   â”œâ”€â”€ login_view.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ progress_view.rs
â”‚   â”‚       â”‚   â””â”€â”€ error_view.rs
â”‚   â”‚       â”œâ”€â”€ branding.rs     # CSS theme loading, logo, org name
â”‚   â”‚       â””â”€â”€ config.rs
â”‚   â”œâ”€â”€ hearth-agent/
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src/
â”‚   â”‚       â”œâ”€â”€ main.rs
â”‚   â”‚       â”œâ”€â”€ control_plane.rs  # Control plane API client
â”‚   â”‚       â”œâ”€â”€ user_env.rs       # User environment lifecycle
â”‚   â”‚       â”œâ”€â”€ activate.rs       # Closure activation logic
â”‚   â”‚       â”œâ”€â”€ heartbeat.rs      # Periodic status reporting
â”‚   â”‚       â”œâ”€â”€ metrics.rs        # Prometheus textfile export
â”‚   â”‚       â”œâ”€â”€ ipc_server.rs     # Unix socket server for greeter
â”‚   â”‚       â””â”€â”€ store.rs          # Nix store interactions
â”‚   â”œâ”€â”€ hearth-enrollment/
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src/
â”‚   â”‚       â”œâ”€â”€ main.rs
â”‚   â”‚       â”œâ”€â”€ hardware.rs     # NixOS Facter integration
â”‚   â”‚       â”œâ”€â”€ network.rs      # Connectivity setup (wired/WiFi)
â”‚   â”‚       â”œâ”€â”€ enroll.rs       # Control plane enrollment protocol
â”‚   â”‚       â”œâ”€â”€ provision.rs    # Disk partitioning + NixOS install
â”‚   â”‚       â””â”€â”€ ui.rs           # ratatui TUI views
â”‚   â””â”€â”€ hearth-common/
â”‚       â”œâ”€â”€ Cargo.toml
â”‚       â””â”€â”€ src/
â”‚           â”œâ”€â”€ lib.rs
â”‚           â”œâ”€â”€ ipc.rs          # AgentRequest/AgentEvent types
â”‚           â”œâ”€â”€ api_client.rs   # Control plane REST client
â”‚           â”œâ”€â”€ config.rs       # Shared config file parsing
â”‚           â””â”€â”€ nix_store.rs    # Store path utilities
â”œâ”€â”€ data/
â”‚   â”œâ”€â”€ hearth-greeter.css      # GTK4 CSS for greeter branding
â”‚   â””â”€â”€ default-logo.svg        # Default organization logo
â”œâ”€â”€ modules/                    # NixOS modules
â”‚   â”œâ”€â”€ agent.nix
â”‚   â”œâ”€â”€ greeter.nix
â”‚   â”œâ”€â”€ pam.nix
â”‚   â”œâ”€â”€ desktop.nix
â”‚   â”œâ”€â”€ hardening.nix
â”‚   â”œâ”€â”€ enrollment.nix
â”‚   â””â”€â”€ roles/
â”‚       â”œâ”€â”€ developer.nix
â”‚       â”œâ”€â”€ designer.nix
â”‚       â”œâ”€â”€ admin.nix
â”‚       â””â”€â”€ default.nix
â”œâ”€â”€ home-modules/               # Home-manager role profiles
â”‚   â”œâ”€â”€ developer.nix
â”‚   â”œâ”€â”€ designer.nix
â”‚   â”œâ”€â”€ admin.nix
â”‚   â”œâ”€â”€ default.nix
â”‚   â””â”€â”€ common.nix              # Shared config (shell, Git, editor baseline)
â”œâ”€â”€ overlays/
â”‚   â””â”€â”€ default.nix             # Adds all Hearth packages to nixpkgs
â”œâ”€â”€ lib/
â”‚   â””â”€â”€ mk-fleet-host.nix       # Helper to define a fleet machine
â”œâ”€â”€ flake.nix
â””â”€â”€ flake.lock
```

### The flake

```nix
# hearth/flake.nix
{
  description = "Hearth â€” Enterprise NixOS Desktop Management";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    home-manager = {
      url = "github:nix-community/home-manager/release-25.05";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    sops-nix = {
      url = "github:Mic92/sops-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    lanzaboote = {
      url = "github:nix-community/lanzaboote";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    comin = {
      url = "github:nlewo/comin";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    attic = {
      url = "github:zhaofengli/attic";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, ... }@inputs:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    craneLib = crane.mkLib pkgs;

    # Shared build configuration for the Rust workspace
    commonArgs = {
      src = craneLib.cleanCargoSource ./.;
      buildInputs = with pkgs; [ gtk4 gdk-pixbuf pango cairo glib openssl ];
      nativeBuildInputs = with pkgs; [ pkg-config wrapGAppsHook4 ];
      strictDeps = true;
    };

    # Build dependencies once â€” cached and reused across all crate builds
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
  in
  {
    # --- Packages ---
    packages.${system} = {
      hearth-greeter = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        cargoExtraArgs = "-p hearth-greeter";
      });

      hearth-agent = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        cargoExtraArgs = "-p hearth-agent";
        # Agent doesn't need GTK â€” override buildInputs
        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
      });

      hearth-enrollment = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        cargoExtraArgs = "-p hearth-enrollment";
        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
      });

      # The enrollment netboot image
      enrollment-image = (nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [ ./modules/enrollment.nix ];
      }).config.system.build.netbootRamdisk;
    };

    # --- Overlay: adds Hearth packages to any nixpkgs ---
    overlays.default = final: prev: {
      hearth-greeter = self.packages.${system}.hearth-greeter;
      hearth-agent = self.packages.${system}.hearth-agent;
      hearth-enrollment = self.packages.${system}.hearth-enrollment;
    };

    # --- NixOS modules ---
    nixosModules = {
      hearth-agent = import ./modules/agent.nix;
      hearth-greeter = import ./modules/greeter.nix;
      hearth-pam = import ./modules/pam.nix;
      hearth-desktop = import ./modules/desktop.nix;
      hearth-hardening = import ./modules/hardening.nix;

      # Meta-module: imports everything a fleet machine needs
      hearth = {
        imports = [
          ./modules/agent.nix
          ./modules/greeter.nix
          ./modules/pam.nix
          ./modules/desktop.nix
          ./modules/hardening.nix
          inputs.comin.nixosModules.comin
          inputs.sops-nix.nixosModules.sops
          inputs.lanzaboote.nixosModules.lanzaboote
        ];
      };
    };

    # --- Home-manager modules for role profiles ---
    homeModules = {
      developer = import ./home-modules/developer.nix;
      designer = import ./home-modules/designer.nix;
      admin = import ./home-modules/admin.nix;
      default = import ./home-modules/default.nix;
      common = import ./home-modules/common.nix;
    };

    # --- Fleet host helper ---
    lib.mkFleetHost = import ./lib/mk-fleet-host.nix inputs;
  };
}
```

### How a fleet machine consumes Hearth

The fleet repository (separate from the Hearth repo) references Hearth as a flake input:

```nix
# fleet-repo/flake.nix
{
  inputs = {
    hearth.url = "github:yourorg/hearth";  # or a private Git URL
    nixpkgs.follows = "hearth/nixpkgs";    # pin to Hearth's nixpkgs
  };

  outputs = { self, hearth, nixpkgs, ... }: {
    nixosConfigurations.ws-0042 = hearth.lib.mkFleetHost {
      hostname = "ws-0042";
      role = "developer";
      hardware = ./hosts/ws-0042/hardware-configuration.nix;
      extraModules = [
        { services.hearth-agent.machineId = "a1b2c3d4-..."; }
      ];
    };
  };
}
```

Machines never reference Hearth's Git repo directly. They consume pre-built closures from the binary cache. The fleet repo's CI builds everything and pushes results to Attic.

### NixOS module: hearth-agent

```nix
# modules/agent.nix
{ config, lib, pkgs, ... }:
let
  cfg = config.services.hearth-agent;
  settingsFormat = pkgs.formats.toml {};
in
{
  options.services.hearth-agent = {
    enable = lib.mkEnableOption "Hearth fleet agent";

    controlPlaneUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://api.hearth.example.com";
      description = "Hearth control plane API endpoint";
    };

    machineId = lib.mkOption {
      type = lib.types.str;
      description = "Machine UUID assigned during enrollment";
    };

    binaryCacheUrl = lib.mkOption {
      type = lib.types.str;
      default = "https://cache.hearth.example.com/fleet-prod";
      description = "Attic binary cache URL for closure pulls";
    };

    roleMapping = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { default = "default"; };
      example = {
        engineering = "developer";
        design = "designer";
        it-admin = "admin";
        default = "default";
      };
      description = "Map of identity provider groups to role profile names";
    };

    roles = lib.mkOption {
      type = lib.types.attrsOf lib.types.path;
      default = {};
      description = "Map of role names to home-manager module paths for pre-built profiles";
    };
  };

  config = lib.mkIf cfg.enable {
    # The agent systemd service
    systemd.services.hearth-agent = {
      description = "Hearth Fleet Agent";
      after = [ "network-online.target" "tailscaled.service" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${pkgs.hearth-agent}/bin/hearth-agent";
        Restart = "always";
        RestartSec = 5;
        # Security hardening
        ProtectSystem = "strict";
        ReadWritePaths = [
          "/nix/var"
          "/var/lib/hearth"
          "/home"
          "/var/lib/prometheus-node-exporter"
        ];
        StateDirectory = "hearth";
      };
    };

    # Agent configuration
    environment.etc."hearth/agent.toml".source = settingsFormat.generate "agent.toml" {
      control_plane.url = cfg.controlPlaneUrl;
      machine.id = cfg.machineId;
      cache.url = cfg.binaryCacheUrl;
      role_mapping = cfg.roleMapping;
    };

    # Ensure nix CLI is available for the agent
    environment.systemPackages = [ pkgs.nix ];
  };
}
```

### NixOS module: hearth-greeter

```nix
# modules/greeter.nix
{ config, lib, pkgs, ... }:
let
  cfg = config.services.hearth-greeter;
in
{
  options.services.hearth-greeter = {
    enable = lib.mkEnableOption "Hearth login greeter";

    branding = {
      organizationName = lib.mkOption {
        type = lib.types.str;
        default = "Your Organization";
        description = "Organization name shown on the login screen";
      };

      logo = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = "Path to SVG or PNG logo. Uses Hearth default if null.";
      };

      cssOverride = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = "Path to custom GTK4 CSS for greeter theming";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    services.greetd = {
      enable = true;
      settings.default_session = {
        command = "${pkgs.hearth-greeter}/bin/hearth-greeter";
        user = "greeter";
      };
    };

    environment.etc."hearth/greeter.toml".source =
      (pkgs.formats.toml {}).generate "greeter.toml" {
        branding = {
          organization_name = cfg.branding.organizationName;
          logo_path = if cfg.branding.logo != null
                      then toString cfg.branding.logo
                      else "${pkgs.hearth-greeter}/share/hearth/default-logo.svg";
          css_path = if cfg.branding.cssOverride != null
                     then toString cfg.branding.cssOverride
                     else "${pkgs.hearth-greeter}/share/hearth/hearth-greeter.css";
        };
        agent_socket = "/run/hearth/agent.sock";
      };

    # Disable GDM â€” we replace it
    services.xserver.displayManager.gdm.enable = lib.mkForce false;
  };
}
```

---

## Binary cache architecture: Attic as the distribution layer

Attic serves as the binary cache infrastructure. Its multi-tenant architecture with content-addressed deduplication makes it ideal for fleet distribution.

### Cache topology

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚              Attic (Central)                     â”‚
â”‚  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚
â”‚  â”‚ hearth-  â”‚ â”‚ fleet-   â”‚ â”‚ fleet-           â”‚ â”‚
â”‚  â”‚ packages â”‚ â”‚ staging  â”‚ â”‚ prod             â”‚ â”‚
â”‚  â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â”‚
â”‚           S3-compatible storage backend          â”‚
â”‚           PostgreSQL metadata store              â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                      â”‚
          â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
          â–¼           â–¼           â–¼
   â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â” â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”
   â”‚ Office A   â”‚ â”‚ Office â”‚ â”‚ Remote â”‚
   â”‚ Nginx      â”‚ â”‚ B      â”‚ â”‚ worker â”‚
   â”‚ cache      â”‚ â”‚ Nginx  â”‚ â”‚(direct)â”‚
   â”‚ proxy      â”‚ â”‚ cache  â”‚ â”‚        â”‚
   â””â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”¬â”€â”€â”€â”€â”˜ â””â”€â”€â”€â”¬â”€â”€â”€â”€â”˜
     â”Œâ”€â”€â”€â”´â”€â”€â”€â”    â”Œâ”€â”€â”€â”´â”€â”€â”    â”Œâ”€â”€â”´â”€â”€â”
     â”‚ws-001 â”‚    â”‚ws-050â”‚    â”‚ws-99â”‚
     â”‚ws-002 â”‚    â”‚ws-051â”‚    â”‚     â”‚
     â”‚...    â”‚    â”‚...   â”‚    â”‚     â”‚
     â””â”€â”€â”€â”€â”€â”€â”€â”˜    â””â”€â”€â”€â”€â”€â”€â”˜    â””â”€â”€â”€â”€â”€â”˜
```

**Three cache tenants** serve distinct purposes:

- `hearth-packages` â€” The Hearth overlay packages (greeter, agent, enrollment). Updated when Hearth itself is updated.
- `fleet-staging` â€” System closures and home-manager closures built from the fleet repo's staging branch. Used for canary testing.
- `fleet-prod` â€” Production-validated closures. Machines in steady-state pull only from this cache.

**Regional Nginx proxy caches** at each office site serve as local CDN nodes. When the first machine in an office pulls a closure, Nginx caches it locally. Subsequent machines pull from the local proxy at LAN speed. Configuration is a standard Nginx reverse proxy with cache:

```nginx
proxy_cache_path /var/cache/nginx/nix levels=1:2
                 keys_zone=nix:10m max_size=100g inactive=30d;

server {
    listen 8080;
    location / {
        proxy_pass https://cache.hearth.example.com;
        proxy_cache nix;
        proxy_cache_valid 200 30d;  # Store paths are immutable â€” cache aggressively
    }
}
```

Nix store paths are content-addressed and immutable â€” once a path exists, its contents never change. This means infinite cache validity for successful responses.

### Machine-side Nix configuration

Fleet machines point their substituters at the Hearth caches, with the public cache.nixos.org as a fallback for upstream nixpkgs packages:

```nix
# Part of the Hearth desktop module
nix.settings = {
  substituters = [
    "https://cache.hearth.example.com/fleet-prod"
    "https://cache.hearth.example.com/hearth-packages"
    "https://cache.nixos.org"
  ];
  trusted-public-keys = [
    "fleet-prod:abc123..."
    "hearth-packages:def456..."
    "cache.nixos.org-1:..."
  ];
  # Prefer the Hearth cache â€” don't fall through to nixos.org unnecessarily
  connect-timeout = 5;
};
```

### Sizing estimates

For a 100-desktop fleet, approximate storage and bandwidth:

- **Central Attic storage**: ~200 GB (deduplication across machines sharing the same role means most paths are stored once)
- **Build server disk**: ~1 TB (build temporaries + local Nix store)
- **Per-update bandwidth**: 200â€“500 MB per machine for a major system update, 5â€“50 MB for config-only changes. Served from local Nginx cache: only 2â€“5 GB fetched from central per update cycle.
- **Per-user home-manager closure**: 100â€“500 MB depending on packages. Most of this is shared with the system closure and already cached.

---

## Pinning and update control

The fleet flake's `flake.lock` pins the exact version of every input: nixpkgs, home-manager, Hearth, and all other dependencies. No machine sees an update that hasn't been explicitly committed, built, and pushed to the production cache.

### Update workflow

1. A developer or automation updates a flake input:
   ```bash
   nix flake update nixpkgs           # Update to latest nixos-25.05
   nix flake update hearth             # Pick up new Hearth version
   ```

2. The change is committed and pushed to the fleet repo:
   ```bash
   git add flake.lock
   git commit -m "chore: update nixpkgs to 25.05.20260215"
   git push origin staging
   ```

3. CI (buildbot-nix) evaluates all machine and user configurations in parallel using `nix-eval-jobs`, builds everything, and pushes to the `fleet-staging` Attic cache.

4. Canary machines (tagged `@canary` in Colmena, or configured to track the staging branch in comin) pick up the update automatically.

5. After validation, merge staging to main. CI builds again (mostly cached), pushes to `fleet-prod`. Production machines pick up the update via comin's Git polling or a Colmena push.

### Hearth-specific vs. nixpkgs updates

Hearth's overlay packages and nixpkgs are versioned independently. A Hearth bug fix (greeter crash, agent protocol change) bumps only the `hearth` input. A security patch in Firefox or OpenSSL comes through the `nixpkgs` input. Both flow through the same CI â†’ staging â†’ production pipeline.

For **nixpkgs itself**, the recommendation is to pin to a stable release branch (`nixos-25.05`) and consume security backports from the NixOS team. Only fork nixpkgs if a specific package issue can't wait for upstream. The fork-and-cherry-pick approach adds significant maintenance burden.

---

## Rust crate dependency summary

| Crate | Binary | Purpose |
|-------|--------|---------|
| `greetd_ipc` | hearth-greeter | greetd protocol IPC |
| `gtk4` (gtk4-rs) | hearth-greeter | Login and progress UI |
| `tokio` | all | Async runtime |
| `axum` | hearth-agent | Unix socket IPC server |
| `reqwest` | hearth-agent, hearth-enrollment | HTTP client for control plane API |
| `sqlx` | hearth-agent | Local SQLite state database |
| `nix-compat` (tvix) | hearth-agent | Nix store path parsing |
| `serde`, `serde_json`, `toml` | all (via hearth-common) | Serialization |
| `ratatui` | hearth-enrollment | Terminal UI |
| `crossterm` | hearth-enrollment | Terminal backend for ratatui |
| `tracing`, `tracing-subscriber` | all | Structured logging |
| `prometheus-client` | hearth-agent | Metrics exposition |

---

## Summary: the full on-device flow

Putting it all together, here is what happens from bare metal to a user working in a fully personalized GNOME environment:

**Enrollment** (once per device):
1. Device PXE boots or boots from USB â†’ loads the enrollment NixOS image into RAM
2. `hearth-enrollment` TUI runs: detects hardware, contacts control plane, displays enrollment code
3. IT admin approves in the web console (or auto-approval matches a pre-registered serial)
4. Control plane generates a NixOS configuration, triggers CI build, pushes closure to Attic
5. Enrollment agent partitions disk via disko, pulls closure, installs NixOS, enrolls Secure Boot + TPM
6. Device reboots into a fully configured NixOS system with `hearth-agent` and `hearth-greeter` running

**First login** (once per user-device pair):
1. `hearth-greeter` shows the branded login screen
2. User enters credentials â†’ greeter drives greetd auth â†’ SSSD authenticates against the identity provider
3. Greeter transitions to "Preparing your workspace..."
4. Greeter asks `hearth-agent` to prepare the environment for this user
5. Agent creates home directory, resolves the user's role from group membership, finds the pre-built role profile in the system closure, activates it (1â€“3 seconds)
6. Agent reports Ready â†’ greeter sends StartSession â†’ GNOME launches with full environment
7. Agent reports the login to the control plane asynchronously. If no per-user closure exists, the control plane generates and builds one.
8. On next login, the per-user closure (if built) is activated instead of the role profile â€” fully personalized

**Subsequent logins** (the steady state):
1. Greeter authenticates â†’ agent finds the per-user closure in the local Nix store â†’ activates in under 1 second â†’ GNOME launches immediately
2. If the control plane has pushed a newer closure (config update), the agent pulls the delta from cache and activates the updated version. Minimal latency.

**System updates**:
1. Fleet repo changes are committed to Git
2. CI builds all affected closures, pushes to Attic
3. comin on each device polls the fleet repo, detects the new configuration, pulls the closure from cache, and runs `nixos-rebuild switch`
4. The next login (or next reboot, depending on the change) picks up the new system configuration
