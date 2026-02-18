# NixOS Enterprise Desktop Platform: Control Plane, Enrollment, and the Home-Manager Solution

**The enrollment flow is the architectural keystone.** By treating device onboarding as an event-driven conversation between a fleet device and a cloud-hosted control plane, we can solve the home-manager problem (issue #5244), eliminate manual provisioning steps, and create a zero-touch experience that rivals Apple's DEP/MDM pipeline â€” while staying fully open-source and NixOS-native. This document details the custom components that must be built, how they interact, and where network boot creates a uniquely powerful onboarding opportunity.

---

## The core insight: enrollment as the home-manager solution

The home-manager problem is well-understood: issue #5244 means the NixOS module mode requires `users.users.<name>` entries at build time, but LDAP/AD/Kanidm users only exist at runtime via NSS. Every existing workaround is a compromise â€” stub users that duplicate identity data, standalone mode that requires per-user activation scripting, or forking home-manager itself.

The control plane enrollment flow sidesteps this entirely by changing *when* and *where* user configurations are generated:

1. A device boots and enrolls with the control plane (establishing machine identity).
2. A user authenticates on the device against the identity provider (Kanidm/AD/FreeIPA).
3. The device agent reports the first-login event to the control plane: "user `jdoe` (groups: `engineering`, `devops`) just authenticated on device `ws-0042`."
4. The control plane queries the identity provider for the user's group memberships and attributes.
5. The control plane generates a concrete home-manager configuration for `jdoe` â€” with their actual username, UID, home directory, and role-appropriate packages/settings â€” and commits it to the fleet repository.
6. CI builds the home-manager closure, pushes it to the binary cache.
7. The device agent pulls the pre-built closure from cache and activates it.

The critical difference from standalone home-manager with login hooks: the configuration is generated *with full knowledge of the user* on the control plane, built in CI like any other NixOS configuration, and delivered as a pre-built closure. No `home-manager switch` runs on the device. No Nix evaluation happens at login time. The user gets a fully configured environment in the time it takes to pull ~200-500 MB from a binary cache â€” comparable to a profile sync, not a build.

This also means home-manager's NixOS module mode works normally on the control plane build, because the control plane *does* know the user at build time. The `users.users.jdoe` entry is generated dynamically into the Nix configuration before evaluation. Issue #5244 ceases to be relevant because we're no longer trying to make home-manager work with runtime-discovered users â€” we discover them on the control plane and generate concrete configs.

---

## Cloud control plane architecture

The control plane runs as a cloud-hosted service (Kubernetes, Nomad, or even a set of NixOS VMs) that fleet devices connect to over the internet. It's the single source of truth for fleet state, identity mapping, configuration generation, and compliance.

### Core services

**API Server** (Go or Rust) â€” The REST API that devices and the web console communicate with. Endpoints cover machine lifecycle (`/api/v1/machines/{id}/enroll`, `/machines/{id}/heartbeat`, `/machines/{id}/report`), user environment management (`/api/v1/users/{id}/environments`), deployment orchestration (`/api/v1/deployments`), policy and compliance (`/api/v1/policies`), and identity integration (`/api/v1/identity/sync`). Modeled after FleetDM's API design but extended for NixOS-specific concepts (generations, closures, store paths, flake URIs).

**Configuration Generator** â€” A service that watches for enrollment and first-login events, queries the identity provider, and produces concrete NixOS/home-manager configurations. This is the most novel component. It maintains a template library of role-based configurations (per-group home-manager modules) and instantiates them with real user data. Output is committed to the fleet Git repository, triggering CI.

**Build Orchestrator** â€” Wraps `nix-eval-jobs` for parallel evaluation and `nix build` for compilation. Manages the build queue, prioritizes first-login builds (user is waiting), and pushes results to Attic. Can run on dedicated build servers with large Nix stores. For a 500-device fleet, 2-4 build servers with 32+ cores and fast NVMe storage are appropriate.

**State Store** â€” PostgreSQL database tracking machine inventory (hardware, enrollment status, current generation, last heartbeat), user-device associations, deployment history, compliance state, and audit log. Every state change is an append-only audit event.

**Identity Bridge** â€” Maintains a synchronized view of the identity provider (Kanidm, FreeIPA, or AD). Watches for group membership changes and triggers re-evaluation of affected user environments. For Kanidm, this uses the REST API at `/v1/`; for FreeIPA, JSON-RPC; for AD, LDAP change notifications or periodic sync.

### Networking: Headscale as the fleet mesh

Devices connect to the control plane over a Headscale (self-hosted Tailscale) mesh network. This solves several problems simultaneously:

- **NAT traversal**: Laptops behind corporate NATs, home networks, or hotel wifi can all reach the control plane without port forwarding or VPN client configuration.
- **Mutual authentication**: Each device gets a WireGuard identity tied to its enrollment. The control plane can verify device identity cryptographically on every connection.
- **Encrypted transport**: All control plane communication travels over WireGuard, regardless of underlying network security.
- **Direct device access**: IT can SSH into any enrolled device via its Headscale address for troubleshooting, without requiring the device to be on the corporate network.

NixOS has a mature `services.headscale` module (48 options) for the server side and `services.tailscale` for clients. The enrollment flow generates a Headscale pre-auth key for each new device, which gets baked into the device's NixOS configuration.

### Web console

Built with Refine + Tremor (or React Admin as fallback), the console provides:

- Fleet dashboard: device inventory, enrollment status, current generation per machine, compliance posture
- User management: identity provider sync status, user-device associations, environment assignment overrides
- Deployment orchestration: staged rollout creation (canary â†’ production), deployment status, one-click rollback
- Compliance: policy definition (SQL-based queries against osquery data, NixOS config assertions), compliance scores, drift detection
- Audit log: every configuration change, deployment, enrollment, and access event with full Git diff links

Non-technical IT admins interact primarily through the console. The underlying Git repository and Nix configurations are implementation details they don't need to see.

---

## The enrollment flow: network boot as the entry point

This is where network boot becomes powerful. Rather than shipping pre-installed machines or requiring manual NixOS installation, devices can PXE/iPXE boot into an enrollment environment that handles the entire onboarding conversation with the control plane.

### Phase 1: Boot and hardware discovery

A new device (or a device being reprovisioned) boots from the network. In an office environment, this is PXE via Pixiecore; for remote workers, a USB stick with a minimal NixOS image that contains only the enrollment agent and Headscale client.

The enrollment environment is a minimal NixOS system (built as a netboot ramdisk via `system.build.netbootRamdisk`) that:

1. Boots into RAM (no disk writes yet).
2. Runs hardware detection via NixOS Facter (successor to `nixos-generate-config`), collecting: CPU, GPU, disk topology, network interfaces, TPM presence, Secure Boot state, serial number, and MAC addresses.
3. Generates a hardware fingerprint â€” a hash of immutable hardware identifiers (serial + TPM EK certificate + motherboard UUID) that uniquely identifies this physical machine.
4. Contacts the control plane API at a well-known endpoint (or discovered via DHCP option, or hardcoded in the enrollment image).

The enrollment image itself is built from the fleet's Nix flake â€” it's a first-class NixOS configuration that includes the enrollment agent, network drivers for expected hardware, and the Headscale client.

### Phase 2: Enrollment negotiation

The enrollment agent sends a registration request to the control plane:

```
POST /api/v1/machines/enroll
{
  "hardware_fingerprint": "sha256:abc123...",
  "serial_number": "PF4GHXYZ",
  "tpm_ek_cert": "base64:...",
  "hardware_report": { /* NixOS Facter output */ },
  "network": {
    "mac_addresses": ["aa:bb:cc:dd:ee:ff"],
    "current_ip": "192.168.1.42"
  }
}
```

The control plane handles this in one of three ways:

**Auto-approve** (for pre-registered devices): If the serial number or hardware fingerprint was pre-loaded (e.g., from a purchase order CSV), the control plane immediately approves enrollment, assigns the device a hostname and role, generates a Headscale pre-auth key, and returns the provisioning instructions.

**Approval queue** (default): The device appears in the web console's enrollment queue. An IT admin reviews the hardware report, assigns a hostname, selects a device role (developer workstation, design workstation, kiosk, shared lab), and clicks "Approve." The control plane then proceeds with provisioning.

**Token-based** (for remote enrollment): The device displays a short enrollment code (6-8 characters) on screen. The user or IT admin enters this code in the web console, which links the device to an enrollment request. This is analogous to the "enter the code shown on your TV" pattern and works for remote workers receiving shipped hardware.

### Phase 3: Provisioning

Once enrolled, the control plane:

1. Generates a complete NixOS configuration for this device: hardware-configuration.nix (from the Facter report), the role-based system config (packages, services, desktop environment), SSSD/Kanidm client configuration pointing to the identity provider, Headscale client configuration with the pre-auth key, the fleet agent (comin or custom) for ongoing management, sops-nix secrets (disk encryption key, WiFi credentials, etc.), and Lanzaboote Secure Boot configuration.
2. Commits this configuration to the fleet Git repository under `hosts/<hostname>/`.
3. Triggers a CI build of the full NixOS system closure.
4. Pushes the closure to Attic (binary cache).
5. Returns the closure store path and cache URL to the enrollment agent.

The enrollment agent then:

1. Partitions and formats the disk via disko (declarative disk config is part of the generated NixOS configuration).
2. Pulls the NixOS closure from the binary cache.
3. Installs NixOS to disk.
4. Enrolls Secure Boot keys (Lanzaboote).
5. Enrolls the TPM for disk encryption (systemd-cryptenroll).
6. Reboots into the installed system.

This entire flow is essentially `nixos-anywhere` decomposed into an API-driven conversation. The enrollment agent does what nixos-anywhere does, but orchestrated by the control plane rather than a human running a CLI command.

### Phase 4: First user login and home-manager activation

The device boots into a fully configured NixOS system with a login screen. No local user accounts exist â€” authentication is handled entirely by SSSD/Kanidm. When a user logs in:

1. PAM authenticates the user against the identity provider via SSSD.
2. `pam_mkhomedir` creates `/home/jdoe` if it doesn't exist.
3. A PAM session hook (or systemd user service triggered by `user@.service`) runs the **user environment agent**.
4. The user environment agent contacts the control plane:

```
POST /api/v1/machines/{machine_id}/user-login
{
  "username": "jdoe",
  "uid": 10042,
  "gid": 10000,
  "groups": ["engineering", "devops", "wheel"],
  "home_directory": "/home/jdoe",
  "first_login": true
}
```

5. The control plane's Configuration Generator:
   - Looks up user `jdoe` in the identity bridge.
   - Maps groups `engineering` + `devops` to the "developer" home-manager profile.
   - Generates a concrete home-manager configuration with `home.username = "jdoe"`, `home.homeDirectory = "/home/jdoe"`, and all the developer role's packages, dotfiles, shell config, Git identity, editor settings, etc.
   - If the user has per-user overrides (e.g., they requested vim instead of the default neovim), those are merged.
   - Commits the configuration and triggers a build.

6. **While the build runs** (typically 1-3 minutes if most packages are cached), the user gets a basic desktop environment. The system-level NixOS config already provides a functional desktop â€” GNOME/KDE/Sway with default settings, a terminal, a browser. The user can start working immediately.

7. When the home-manager closure finishes building and lands in the binary cache, the user environment agent pulls it and activates it. The desktop environment reloads with the user's personalized configuration. A notification tells the user their environment is ready.

**Subsequent logins** are instant: the agent checks if the user's home-manager closure is already in the local Nix store (it will be, unless the config was updated). If so, it activates immediately â€” just symlinking, which takes under a second. If the control plane has a newer version, it pulls the delta from the binary cache (typically a few MB for config changes, larger for new packages).

---

## The user environment agent: technical detail

This is the most critical custom component. It runs as a systemd user service on each device.

### Activation flow

```
login â†’ PAM auth â†’ pam_mkhomedir â†’ systemd user@.service starts â†’
  user-env-agent.service starts â†’
    check local cache for user's closure â†’
    if cached and current: activate (symlink, <1s) â†’
    if not cached: pull from binary cache â†’ activate (10-60s depending on size) â†’
    if no closure exists yet: notify control plane â†’ wait for build â†’ pull â†’ activate
```

### Pre-built role profiles vs. per-user generation

Two strategies are possible, and the architecture supports both:

**Pre-built role profiles** (fast, less personalized): At system build time, a fixed set of home-manager profiles are built as part of the NixOS closure â€” one for each role (developer, designer, admin, default). These use a template username that gets substituted at activation time. This requires a custom activation script that does path rewriting, but avoids any control plane round-trip on first login.

The technical challenge is that home-manager activation scripts hardcode `$HOME` paths. A pre-built profile for user "template" contains symlinks pointing to `/home/template/.config/...`. For user `jdoe`, these need to point to `/home/jdoe/.config/...`. This can be handled by a thin wrapper that:

- Copies the activation script from the store
- Does `sed` replacement of the template paths with the actual user's paths
- Runs the modified activation

This is fragile â€” it breaks if home-manager changes its activation script format. But it's fast (~1 second).

**Per-user generation on control plane** (slower first login, fully personalized): The approach described in the enrollment flow above. More robust because the generated configuration is a real home-manager config for the real user â€” no path rewriting hacks. First login takes 1-3 minutes for the build (user gets a functional default desktop immediately), subsequent logins are instant.

**Recommended approach**: Use pre-built role profiles as a fast fallback that activates immediately on first login, then seamlessly upgrade to the per-user configuration when it finishes building on the control plane. The user gets a functional personalized environment in under a second, and a fully personalized one within a few minutes.

### Offline operation

The agent must handle offline scenarios (laptop taken on a plane):

- If the user has logged in before on this device, their home-manager closure is in the local Nix store. Activation works entirely offline.
- If the user has never logged in on this device and it's offline, fall back to the pre-built role profile (which is part of the system closure and always available locally).
- SSSD credential caching handles offline authentication.
- When connectivity returns, the agent syncs with the control plane, pulls any updates, and activates the per-user closure if it differs from the role-based fallback.

---

## Network boot: deeper exploration

Network boot isn't just for initial enrollment â€” it can serve as an ongoing operational tool.

### PXE/iPXE boot service on the control plane

The control plane runs a boot service (extending Pixiecore's model) that serves different boot images based on device identity:

**Unknown device** â†’ Enrollment image (minimal NixOS with enrollment agent)

**Known device, needs reprovisioning** â†’ Full NixOS installer closure (triggered by IT admin via console, e.g., after hardware replacement or security incident)

**Known device, normal boot** â†’ `exit` (tells iPXE to boot from local disk). This is the steady-state â€” devices normally boot from disk, not the network.

**Known device, recovery** â†’ NixOS rescue image with diagnostic tools and fleet agent

The boot service uses iPXE's ability to chain-load scripts from HTTP endpoints. The device's MAC address or SMBIOS serial number is passed as a query parameter:

```
#!ipxe
dhcp
chain https://boot.fleet.example.com/v1/boot?mac=${mac}&serial=${serial}&asset=${asset}
```

The control plane's `/v1/boot` endpoint looks up the device and returns the appropriate iPXE script dynamically. This is the same pattern used by Equinix Metal and Hivelocity for bare-metal provisioning, adapted for fleet management.

### Diskless and hybrid modes

For specific use cases, ongoing network boot (not just enrollment) is valuable:

**Lab/classroom machines**: Boot diskless from the network every time. The NixOS system lives in a squashfs ramdisk (5-8 GB for a desktop config). Every reboot returns the machine to a known-good state. User data lives on network storage (NFS/CIFS via pam_mount). The control plane's boot service points these machines at the latest squashfs image in the binary cache.

**High-security environments**: Combine diskless boot with impermanence. The machine has no persistent local storage at all â€” everything comes from the network. No data exfiltration via stolen hard drives. The TPM attests the boot chain, the control plane verifies attestation before releasing secrets.

**Hybrid**: Standard disk-installed NixOS for daily use, but the GRUB menu includes a "network recovery" option that iPXE boots from the control plane. IT can remotely trigger this via `grub-reboot` + reboot command (the pattern Carlos Vaz documented for managing ~100 lab machines) to reprovision a machine without physical access.

### Network boot + enrollment at scale

For deploying 50+ machines simultaneously (new office, lab refresh):

1. Set up a local Pixiecore instance (or configure DHCP to point at the control plane's boot service).
2. Pre-register serial numbers in the control plane (from purchase order).
3. Power on all machines. They PXE boot, hit the enrollment endpoint, get auto-approved, and self-provision in parallel.
4. The control plane's build orchestrator parallelizes across all available build capacity.
5. Machines pull closures from the binary cache (with a local Nginx cache to avoid redundant downloads â€” all machines in the same role pull identical closures).
6. Provisioning completes in 15-30 minutes for a batch of 50 identical machines.

---

## Device identity and trust

Every enrollment flow needs a trust anchor â€” how does the control plane know this device is legitimate?

### TPM-based device identity

Modern enterprise laptops (ThinkPad, Dell Latitude, HP EliteBook, Framework) all have TPM 2.0 chips. NixOS has solid TPM support (`security.tpm2.enable = true` with full PKCS#11 integration). The enrollment flow uses TPM in two ways:

**Endorsement Key (EK) certificate**: The TPM's EK cert is unique per chip and signed by the manufacturer. During enrollment, the agent extracts this cert and sends it to the control plane. The control plane can verify it against the manufacturer's CA to confirm this is a genuine TPM (not a VM pretending to have one). This establishes hardware identity.

**Attestation**: After enrollment, the device can use TPM-based remote attestation to prove to the control plane that it booted a known-good NixOS configuration (measured boot). The PCR values attest that the kernel, initrd, and boot parameters match what the control plane deployed. Combined with Lanzaboote Secure Boot, this creates a strong chain of trust from firmware to userspace.

### Machine identity bootstrapping

The "first secret" problem â€” how does a new machine get its initial credentials to talk to the control plane? â€” is solved by the enrollment flow:

1. The enrollment image contains only the control plane's public URL and CA certificate. No secrets.
2. The enrollment agent generates an ephemeral key pair during enrollment.
3. The control plane, upon approving enrollment, generates: a Headscale pre-auth key (one-time use), an age public/private key pair for sops-nix, and an SSH host key.
4. These secrets are encrypted to the device's ephemeral public key and returned in the enrollment response.
5. During provisioning (Phase 3), these secrets are written to disk and the ephemeral key is destroyed.
6. Post-provisioning, the device's age key serves as its long-term identity for sops-nix secrets, and its SSH host key identifies it for deploy-rs/Colmena operations.

This means no pre-shared secrets, no USB key distribution, no manual key ceremony. The trust chain is: physical access to the device (enrollment) â†’ TPM-attested hardware â†’ control plane-issued credentials.

---

## Data model for the control plane

The core entities:

```
Machine {
  id: UUID
  hostname: string
  hardware_fingerprint: sha256
  serial_number: string
  tpm_ek_cert_fingerprint: sha256
  enrollment_status: pending | approved | provisioned | decommissioned
  headscale_node_id: string
  current_generation: int
  current_system_closure: nix_store_path
  target_system_closure: nix_store_path  // null if up-to-date
  last_heartbeat: timestamp
  hardware_report: jsonb  // NixOS Facter output
  role: string  // "developer", "designer", "kiosk", etc.
  tags: string[]  // for Colmena-style group operations
  fleet_config_git_ref: string  // commit hash of current config
}

UserEnvironment {
  id: UUID
  username: string
  uid: int
  machine_id: UUID  // FK to Machine
  identity_provider_id: string  // user's ID in Kanidm/AD
  groups: string[]
  profile_role: string  // resolved from groups
  hm_closure: nix_store_path  // built home-manager closure
  hm_config_git_ref: string
  status: pending_build | building | ready | active | stale
  activated_at: timestamp
}

Deployment {
  id: UUID
  strategy: canary | staged | immediate | rollback
  target_filter: jsonb  // tag-based machine selection
  target_closure: nix_store_path
  git_ref: string
  status: queued | building | deploying | completed | failed | rolled_back
  machines_total: int
  machines_completed: int
  machines_failed: int
  created_by: user_id
  started_at: timestamp
  completed_at: timestamp
}

CompliancePolicy {
  id: UUID
  name: string
  description: string
  severity: critical | high | medium | low | info
  query_type: osquery | nix_assertion | config_drift
  query: string  // SQL for osquery, Nix expression for assertions
  auto_remediation: boolean
  remediation_action: string  // e.g., "trigger deployment of latest config"
}

AuditEvent {
  id: UUID
  timestamp: timestamp
  actor: string  // user or system
  action: string
  resource_type: string
  resource_id: UUID
  details: jsonb
  git_diff_url: string  // link to the config change if applicable
}
```

---

## What must be built vs. what exists

| Component | Status | Effort estimate |
|-----------|--------|----------------|
| Enrollment image (NixOS netboot + agent) | Custom build, uses existing NixOS netboot primitives | 2-3 weeks |
| Enrollment agent (runs in enrollment image) | Build from scratch | 3-4 weeks |
| Control plane API server | Build from scratch | 8-12 weeks |
| Configuration generator | Build from scratch (most novel component) | 6-8 weeks |
| Build orchestrator | Wraps nix-eval-jobs + nix build, custom queue | 3-4 weeks |
| Identity bridge (Kanidm/AD sync) | Build from scratch, Kanidm REST API is well-documented | 3-4 weeks |
| User environment agent (on-device) | Build from scratch | 4-6 weeks |
| Web console (Refine + Tremor) | Build from scratch | 8-12 weeks |
| Boot service (iPXE/Pixiecore extension) | Extend existing Pixiecore, add API-driven dispatch | 2-3 weeks |
| Headscale integration | Configuration + enrollment flow, uses existing module | 1-2 weeks |
| Fleet/osquery integration | Configuration + custom NixOS tables | 2-3 weeks |
| Attic (binary cache) | Exists, deploy and configure | 1 week |
| sops-nix (secrets) | Exists, integrate with enrollment flow | 1-2 weeks |
| comin (fleet GitOps agent) | Exists, configure for fleet use | 1 week |
| Colmena (ad-hoc deployment) | Exists, integrate with control plane | 1 week |
| Lanzaboote + TPM (Secure Boot/FDE) | Exists, integrate with enrollment flow | 1-2 weeks |
| Prometheus/VictoriaMetrics/Grafana | Exists, NixOS modules are mature | 1-2 weeks |

**Total estimated effort for core platform**: 40-60 weeks of engineering, or 3-4 engineers for ~4-5 months to reach a functional MVP. The first milestone (enrollment + provisioning + basic console) could ship in 8-10 weeks with 2 engineers.

---

## Development phasing

**Phase 1 â€” Enrollment and provisioning** (weeks 1-10): Build the enrollment image, enrollment agent, boot service, and minimal control plane API. A device can PXE boot, enroll, get provisioned with NixOS, and join the Headscale mesh. No web console yet â€” enrollment approval via CLI.

**Phase 2 â€” User environments** (weeks 6-16, overlapping): Build the user environment agent, configuration generator, and identity bridge. First-login triggers home-manager generation on the control plane. Pre-built role profiles provide instant fallback.

**Phase 3 â€” Console and operations** (weeks 10-22): Build the web console, deployment orchestration, and compliance integration. IT admins can manage the fleet without touching Nix code. Fleet/osquery provides endpoint telemetry.

**Phase 4 â€” Hardening and scale** (weeks 18-28): TPM attestation, compliance policy engine, STIG/CIS module development, performance optimization for 500+ device fleets, documentation.

---

## Open questions and risks

**Build latency for first-login user environments**: The 1-3 minute build time for a user's home-manager config on first login may be acceptable (the user has a functional desktop immediately), but it depends on organizational expectations. Aggressive binary cache warming (pre-building all role profiles nightly) reduces this to cache-pull time only (~30 seconds).

**Configuration generator complexity**: Dynamically generating valid Nix configurations from identity provider data is the hardest engineering problem. The template system needs to be expressive enough for real organizational needs (per-team package lists, project-specific tooling, individual preferences) without becoming its own DSL. Starting with simple group-to-profile mapping and iterating is the right approach.

**Nix evaluation on the control plane**: The control plane needs Nix installed and a substantial nixpkgs checkout to evaluate configurations. This is a non-trivial operational dependency. FlakeHub's "pre-evaluated store paths" approach is interesting here â€” if the control plane could skip evaluation and go directly to builds, it would be significantly faster. But that introduces vendor dependency.

**iPXE/PXE limitations for remote workers**: Network boot works beautifully in office environments with local DHCP, but remote workers can't PXE boot from a cloud control plane. The USB enrollment stick is the fallback, but it requires shipping physical media (or having the user create one). An alternative is a pre-installed minimal NixOS with just the enrollment agent â€” shipped on commodity hardware from the vendor, with the enrollment agent completing provisioning on first boot over the internet.

**Scale ceiling for centralized builds**: At 1,000+ devices with frequent user environment rebuilds, the build infrastructure needs to be substantial. Distributed building (Nix's `--builders` flag to distribute builds across multiple machines) and aggressive caching mitigate this, but it's a real operational concern. The binary cache deduplication in Attic helps â€” identical packages across users are stored once.
