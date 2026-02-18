# Hearth Data Architecture: Hybrid Git + Database Source of Truth

**The fleet Git repo as a write-heavy store of generated Nix files is an anti-pattern.** Hearth splits the source of truth into two layers: Git for the module library (actual Nix code that benefits from review, branching, and diffing) and PostgreSQL for instance data (machine inventory, user-device associations, per-host parameters, deployment state). At build time, the control plane exports instance data as JSON and evaluates it against the module library â€” preserving Nix's purity guarantees while eliminating the impedance mismatch of forcing transactional mutations through Git commits.

This document also retires **comin** from the architecture. With a cloud control plane that centrally builds all closures and an on-device agent that already communicates with that control plane, a separate GitOps reconciliation loop is redundant complexity. The hearth-agent handles fleet updates directly by polling the control plane for new closures and pulling them from the binary cache.

---

## The problem with Git as the universal source of truth

The earlier architecture had the control plane committing generated Nix files to a fleet Git repository every time a machine enrolled or a user logged in for the first time. This creates several problems:

**Git is a terrible write-heavy transactional store.** Two machines enrolling simultaneously produce merge conflicts. The control plane needs locking or retry logic around `git commit && git push`. At fleet scale (500+ devices, frequent user logins), the commit rate becomes a bottleneck.

**Generated Nix boilerplate pollutes history.** A `hosts/ws-0042/default.nix` that sets `networking.hostName = "ws-0042"` and `services.hearth-agent.machineId = "a1b2c3d4"` isn't meaningfully reviewable code. It's data serialized as Nix syntax. Thousands of commits that amount to "enrolled a machine" or "generated user environment for jdoe" bury the actual configuration changes that matter.

**Querying fleet state requires Nix evaluation or file parsing.** "Which machines are running the developer role?" shouldn't require evaluating a Nix flake or grepping through files. It's a database query.

**The web console wants API-driven mutations.** When an IT admin changes a machine's role in the console, that mutation must flow through Git â€” generating a Nix file, committing, pushing, waiting for CI. The database can handle this transactionally in milliseconds.

**Instance data doesn't use Nix language features.** A machine's hostname, ID, role assignment, and tag list are flat key-value data. They never use `mkIf`, `mkMerge`, imports, or any of the module system's expressive power. Serializing them as `.nix` files gains nothing over JSON.

---

## What belongs where

### Git: the module library

Git remains the source of truth for everything that is genuinely *code* â€” configuration that benefits from review, branching, blame, and diffing:

- **Hearth NixOS modules**: agent.nix, greeter.nix, pam.nix, desktop.nix, hardening.nix
- **Home-manager role profiles**: developer.nix, designer.nix, admin.nix, default.nix, common.nix
- **Hardware profiles**: reusable configurations for known hardware platforms (ThinkPad T14s, Framework 16, Dell Latitude, etc.)
- **Compliance and hardening modules**: STIG mappings, CIS controls, security baselines
- **The flake.nix and flake.lock**: dependency pinning, module composition logic, the parameterized build functions
- **Overlays and package customizations**: any nixpkgs modifications the organization needs
- **Extra modules**: optional feature sets (VPN configuration, printer setup, specific development toolchains)

Changes to the module library flow through merge requests with code review, are tested in CI, and are deployed through the standard staging â†’ production pipeline. This is the workflow Git was designed for.

### Database (PostgreSQL): instance data

The control plane's PostgreSQL database is the source of truth for all fleet *state* â€” data that is created, updated, and queried through API operations:

```
Machine {
  id: UUID
  hostname: string
  role: string                      -- "developer", "designer", "kiosk"
  tags: string[]                    -- for group operations: ["office-nyc", "canary"]
  hardware_profile: string          -- selects a hardware module from the library
  hardware_fingerprint: sha256
  serial_number: string
  tpm_ek_cert_fingerprint: sha256
  enrollment_status: enum
  headscale_node_id: string
  current_closure: nix_store_path   -- what's currently running
  target_closure: nix_store_path    -- what should be running (null if current)
  current_generation: int
  module_library_ref: string        -- git commit of the module library used for current build
  instance_data_hash: sha256        -- hash of the JSON used for current build
  extra_modules: string[]           -- optional feature modules to include
  extra_config: jsonb               -- per-machine overrides (static IP, custom mounts, etc.)
  last_heartbeat: timestamp
  hardware_report: jsonb
}

UserEnvironment {
  id: UUID
  username: string
  uid: int
  gid: int
  machine_id: UUID
  identity_provider_id: string
  groups: string[]
  resolved_role: string
  hm_closure: nix_store_path
  module_library_ref: string
  instance_data_hash: sha256
  status: enum
  activated_at: timestamp
}

Deployment {
  id: UUID
  strategy: enum
  target_filter: jsonb              -- tag/role-based machine selection
  module_library_ref: string        -- git commit being deployed
  status: enum
  machines_total: int
  machines_completed: int
  machines_failed: int
  created_by: string
  started_at: timestamp
  completed_at: timestamp
}
```

Every mutation to this data is an API call, immediately queryable, and recorded in the append-only `AuditEvent` table. The web console reads and writes this data directly. No Git ceremony.

---

## The build contract

A build is a pure function of two immutable inputs:

```
closure = build(module_library @ git_ref, instance_data_json)
```

**Module library git ref**: a specific commit hash of the Hearth + fleet module repository. This pins every NixOS module, home-manager profile, overlay, and `flake.lock` dependency.

**Instance data JSON**: a snapshot of the machine or user record from the database, exported as a JSON file that Nix consumes via `builtins.fromJSON`.

Both inputs are recorded with every build. Any build can be reproduced by checking out the recorded git ref and supplying the recorded instance data. Nix evaluation is pure â€” no `--impure`, no `builtins.exec`, no network calls during eval.

### How the module library consumes instance data

The flake exposes parameterized builder functions rather than per-host `nixosConfigurations`:

```nix
# flake.nix (module library)
{
  outputs = { self, nixpkgs, hearth, home-manager, ... }: {

    lib.buildMachineConfig = { instanceDataPath, system ? "x86_64-linux" }:
      let
        machine = builtins.fromJSON (builtins.readFile instanceDataPath);
        pkgs = nixpkgs.legacyPackages.${system};
      in
      nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          hearth.nixosModules.hearth
          ./roles/${machine.role}.nix
          ./hardware-profiles/${machine.hardware_profile}.nix

          # Instance data â†’ NixOS options
          {
            networking.hostName = machine.hostname;
            services.hearth-agent = {
              machineId = machine.id;
              roleMapping = machine.role_mapping;
              controlPlaneUrl = machine.control_plane_url;
              binaryCacheUrl = machine.binary_cache_url;
            };
          }

          # Per-machine extra modules (VPN, printers, dev toolchains)
          ] ++ (map (m: ./extras/${m}.nix) (machine.extra_modules or []))
          ++ [

          # Per-machine config overrides from the database
          (lib.mkIf (machine ? extra_config) (
            builtins.fromJSON (builtins.toJSON machine.extra_config)
          ))
        ];
      };

    lib.buildUserEnv = { userDataPath, system ? "x86_64-linux" }:
      let
        user = builtins.fromJSON (builtins.readFile userDataPath);
        pkgs = nixpkgs.legacyPackages.${system};
      in
      home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        modules = [
          ./home-modules/${user.role}.nix
          {
            home.username = user.username;
            home.homeDirectory = "/home/${user.username}";
            home.stateVersion = "25.05";
          }
        ];
      };
  };
}
```

The full Nix DSL is available in the role modules, hardware profiles, and extras â€” conditionals, `mkMerge`, typed options, imports, overlays. Only the leaf-level parameterization (hostname, machine ID, username, role selection) comes from the JSON. This is the right division of labor: Nix handles composition and policy, the database handles identity and inventory.

### Build orchestrator workflow

When the control plane needs to build a machine configuration:

```
1. Determine the current module library ref (the git commit to build against)
2. Export the machine's instance data from PostgreSQL as JSON
3. Write it to a temporary build directory
4. Invoke: nix build --expr '
     let flake = builtins.getFlake "git+ssh://git.example.com/fleet-modules?ref=<git_ref>";
     in (flake.lib.buildMachineConfig {
       instanceDataPath = /tmp/build-abc123/machine.json;
     }).config.system.build.toplevel'
5. Record: (git_ref, sha256(machine.json), result store path) in the database
6. Push the closure to Attic
```

For user environments, the same pattern with `buildUserEnv` and the user's JSON export.

### The extra_config escape hatch

The `extra_config` JSONB field on the Machine record allows IT admins to set arbitrary NixOS options through the web console without touching Git. This is intentionally limited â€” it's for per-machine overrides like static IP addresses, custom mount points, or specific package additions. Structural changes to what a "developer workstation" means still go through the module library in Git.

The console UI should expose `extra_config` as a structured form for common overrides (networking, extra packages, specific services) rather than a raw JSON editor. Validation happens both in the console (schema checking) and during Nix evaluation (NixOS module type system catches invalid options).

---

## Fleet updates without comin

### Why comin is retired

comin's design assumes each device independently polls a Git repo, evaluates `nixosConfigurations.<hostname>`, and rebuilds locally. In the Hearth architecture, this creates several problems:

**Instance data isn't in Git.** comin can't build a machine configuration without the JSON that the database provides. We'd have to materialize the JSON into Git as a transport mechanism, defeating the purpose of the database.

**Local evaluation is wasteful.** Every device evaluating the same Nix flake independently duplicates CPU time that the control plane's build servers handle once. For a 500-device fleet, that's 500 evaluations of the same module library instead of one centralized parallel evaluation via `nix-eval-jobs`.

**The agent already talks to the control plane.** hearth-agent maintains a persistent connection to the control plane for heartbeats, user environment management, and enrollment. Adding update polling to this existing channel is trivial. Running a separate GitOps daemon alongside is redundant.

**comin doesn't integrate with deployment orchestration.** The control plane needs to orchestrate staged rollouts (canary â†’ production), track deployment progress, and trigger rollbacks. comin's independent polling model doesn't participate in this orchestration â€” each machine updates on its own schedule.

### The hearth-agent update model

The agent handles fleet updates through a simple poll-and-pull loop:

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”         â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”         â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚ hearth-agent â”‚â—„â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚  Control Plane    â”‚â”€â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚  Attic   â”‚
â”‚ (on device)  â”‚  HTTPS  â”‚  API Server       â”‚  push   â”‚  (cache) â”‚
â”‚              â”‚         â”‚                   â”‚         â”‚          â”‚
â”‚ poll: "any   â”‚         â”‚ responds with     â”‚         â”‚          â”‚
â”‚  new closure â”‚         â”‚ target closure    â”‚         â”‚          â”‚
â”‚  for me?"    â”‚         â”‚ path + cache URL  â”‚         â”‚          â”‚
â”‚              â”‚         â”‚ (or "up to date") â”‚         â”‚          â”‚
â”‚ pull closure â”‚â—„â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚          â”‚
â”‚ from cache   â”‚  nix copy --from                     â”‚          â”‚
â”‚              â”‚                                      â”‚          â”‚
â”‚ activate     â”‚                                      â”‚          â”‚
â”‚ nixos-rebuildâ”‚                                      â”‚          â”‚
â”‚ switch       â”‚                                      â”‚          â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                                      â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

#### Polling and notification

The agent polls the control plane on a configurable interval (default: 60 seconds):

```
GET /api/v1/machines/{machine_id}/target-state
Authorization: Bearer <device-token>

Response:
{
  "current_closure": "/nix/store/abc123-nixos-system-ws-0042",
  "target_closure": "/nix/store/def456-nixos-system-ws-0042",  // null if up-to-date
  "cache_url": "https://cache.hearth.example.com/fleet-prod",
  "deployment_id": "d1e2f3...",
  "strategy": "immediate",        // or "scheduled", "manual"
  "scheduled_at": null,
  "rollback_closure": "/nix/store/abc123-nixos-system-ws-0042"
}
```

When the control plane has a new target closure, the agent:

1. Pulls the closure from the binary cache: `nix copy --from <cache_url> <target_closure>`
2. Activates it: `nixos-rebuild switch --install-bootloader` (equivalent to what comin did)
3. Reports the result back to the control plane:

```
POST /api/v1/machines/{machine_id}/report-update
{
  "deployment_id": "d1e2f3...",
  "status": "success",           // or "failed"
  "new_generation": 47,
  "active_closure": "/nix/store/def456-nixos-system-ws-0042",
  "error": null
}
```

For latency-sensitive deployments, the control plane can also push a notification over the Headscale mesh (a lightweight gRPC stream or SSE connection) to wake the agent immediately rather than waiting for the next poll cycle.

#### Staged rollouts

The control plane orchestrates staged deployments by controlling which machines receive a target closure and when:

**Canary phase**: The control plane sets `target_closure` only for machines tagged `canary`. Those agents poll, pull, activate, and report back. The deployment's `machines_completed` / `machines_failed` counters update in real time.

**Validation window**: The control plane waits for a configurable period (e.g., 30 minutes) after all canary machines report success. Monitoring alerts (Prometheus/Grafana) can fail the deployment during this window.

**Production rollout**: If the canary phase passes, the control plane sets `target_closure` for the remaining machines. It can stagger this â€” setting targets for 10% of production machines at a time, waiting for success, then the next 10% â€” or release to all at once.

**Automatic rollback**: If any machine reports `status: "failed"`, or if a machine becomes unreachable after activation (missed heartbeats), the control plane can automatically set `target_closure` back to `rollback_closure` for affected machines. This provides the safety guarantee that deploy-rs's "magic rollback" offered, but orchestrated from the control plane rather than a local timer.

#### Update behavior configuration

```toml
# /etc/hearth/agent.toml

[updates]
# How often to check for updates
poll_interval_seconds = 60

# When to apply updates
# "immediate" - apply as soon as available
# "maintenance_window" - only apply during specified hours
# "manual" - require user confirmation via desktop notification
apply_strategy = "immediate"

# Maintenance window (if apply_strategy = "maintenance_window")
maintenance_window_start = "02:00"
maintenance_window_end = "05:00"
maintenance_window_days = ["Mon", "Tue", "Wed", "Thu", "Fri"]

# Reboot behavior
# "if_needed" - reboot only if kernel/initrd changed
# "always" - reboot after every system update
# "never" - switch without rebooting (user reboots at their convenience)
reboot_policy = "if_needed"

# Desktop notification before reboot
notify_before_reboot = true
reboot_delay_seconds = 300    # 5-minute warning
```

The `apply_strategy = "manual"` mode deserves attention for workstation fleets. Unlike servers, desktops have users actively working. The agent can send a D-Bus notification: "A system update is available. Apply now?" with Snooze / Apply / Apply at Shutdown options. This is closer to the macOS/Windows update experience that enterprise users expect.

#### Offline resilience

When the device can't reach the control plane:

- The agent continues running on its last known-good configuration.
- Poll attempts fail silently and retry on the next interval.
- User environment activation works entirely from the local Nix store (no control plane needed for cached closures).
- Heartbeats and update reports queue locally (SQLite) and flush when connectivity returns.
- The control plane marks the machine as "last seen: X hours ago" in the fleet dashboard but doesn't take any action. The machine is still running a known, valid NixOS configuration.

When connectivity restores, the agent catches up: sends queued heartbeats, checks for a target closure, and applies any pending updates. If multiple updates were released while offline, the agent jumps directly to the latest target â€” no need to apply intermediate versions.

---

## Reproducibility guarantees

The hybrid model preserves full reproducibility. Every build is recorded as:

| Field | Value |
|-------|-------|
| `module_library_ref` | `git:abc123def` â€” commit hash of the module library |
| `instance_data_hash` | `sha256:789xyz...` â€” hash of the JSON used |
| `flake_lock_hash` | Implicit in the git ref (flake.lock is committed) |
| `result_closure` | `/nix/store/...` â€” the output store path |
| `built_at` | Timestamp |
| `nixpkgs_rev` | Extracted from flake.lock for audit purposes |

To reproduce a build: check out `module_library_ref`, retrieve the instance data snapshot from the audit log, place it as a JSON file, and run `nix build`. The result will be the same store path (Nix's content-addressing guarantees this for identical inputs).

The database keeps instance data snapshots as part of the audit trail. When a machine record changes (role reassignment, new extra_config), the old state is preserved in `AuditEvent.details` as the "before" snapshot. This is the equivalent of Git history for instance data, but queryable and not cluttered with generated boilerplate.

---

## Impact on other components

### Web console

The console operates entirely against the database via the REST API. Creating a machine, changing a role, adding tags, assigning overrides â€” all database mutations. The console never touches Git. It does display the current module library ref and can link to the Git hosting UI for code changes, but fleet operations are API-driven.

### CI pipeline

CI triggers on two events:

1. **Module library commit** (Git push to main/staging): Rebuild all machine configurations and user environments against the new code. This is the "deploy new policy" path. CI exports every machine's instance data from the database, evaluates in parallel with `nix-eval-jobs`, and pushes results to Attic.

2. **Control plane API trigger** (machine enrolled, user first-login, role change, extra_config update): Rebuild the affected machine or user environment against the current module library ref. This is the "deploy new instance" path. Only the affected configurations are rebuilt.

Both paths produce closures in Attic and update the `target_closure` field in the database. The agent poll loop handles the rest.

### Secrets (sops-nix)

sops-nix encrypted secret files remain in Git â€” they're part of the module library. Machine-specific secret access is controlled by `.sops.yaml` rules that reference the machine's age public key. When a new machine enrolls, the control plane adds its age key to `.sops.yaml` and runs `sops updatekeys` â€” this *is* a Git commit, but it's a small, well-defined one (updating a YAML file), not generating Nix code.

An alternative for future consideration: move secret distribution to the control plane API entirely, with the agent fetching decrypted secrets over the authenticated Headscale channel. This would eliminate the Git dependency for secrets but requires careful security analysis (the control plane becomes a high-value target holding decrypted secrets in transit).

### Colmena

Colmena remains available for ad-hoc operations, but its role shrinks. Since it expects `nixosConfigurations.<hostname>` in a flake, it would need a wrapper that generates these from the database at invocation time. More practically, ad-hoc operations (push an emergency fix to a specific machine) can go through the control plane API: set the machine's `target_closure` with `strategy: "immediate"` and let the agent handle it. Colmena becomes a break-glass tool for situations where the control plane itself is down.

---

## Migration from the earlier architecture

For teams already following the "fleet Git repo with per-host Nix files" pattern from the earlier architecture documents:

1. **Keep the module library in Git.** Factor out role definitions, hardware profiles, extras, and shared configuration into a clean module library repo.

2. **Import existing hosts into the database.** A migration script reads `hosts/*/default.nix`, extracts the instance data (hostname, role, machine ID, tags), and creates Machine records in PostgreSQL.

3. **Delete generated per-host Nix files from Git.** The module library repo should contain only reusable modules and the parameterized builder functions.

4. **Switch agents from comin to hearth-agent update polling.** Roll this out as a standard NixOS configuration change â€” the Hearth NixOS module replaces comin's systemd service with the agent's update loop.

5. **Verify reproducibility.** For each migrated machine, confirm that `build(module_library @ HEAD, exported instance data)` produces the same closure path as the old per-host configuration.

---

## Summary

| Concern | Old model | New model |
|---------|-----------|-----------|
| Machine configuration definition | Per-host `.nix` files committed to Git | Parameterized builder function + instance data from database |
| User environment generation | Commit generated home-manager config to Git | Export user record as JSON, build against module library |
| Fleet state queries | Evaluate Nix or parse files | SQL queries against PostgreSQL |
| Web console mutations | Generate Nix â†’ Git commit â†’ push â†’ CI | API call â†’ database write â†’ trigger build |
| Fleet updates | comin polls Git, evaluates locally | hearth-agent polls control plane, pulls pre-built closures |
| Staged rollouts | comin branch tracking (limited) | Control plane orchestrates canary â†’ production |
| Rollback | Manual comin branch switch | Control plane sets rollback_closure, agent applies |
| Offline operation | comin works if Git is cached locally | Agent runs on last closure, queues heartbeats |
| Reproducibility | Git commit = build input | (Git ref + instance data hash) = build input |
| Audit trail | Git history (cluttered with generated files) | Append-only database events + clean Git history for code |
| Secrets | sops-nix files in Git (unchanged) | sops-nix files in Git (unchanged for now) |
