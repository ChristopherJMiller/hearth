# Hearth Demo Walkthrough

## Prerequisites

- Docker and Docker Compose
- Nix with flakes enabled
- ~10 GB free disk space, ~8 GB RAM for all services

## Quick Start

```bash
nix develop
just demo
just host-aliases   # one-time, adds *.hearth.local to /etc/hosts (sudo)
```

Wait ~3 minutes for `just demo` to bring up all services. The API server starts last and prints service URLs.

`just host-aliases` only needs to run once per machine — it adds the
`*.hearth.local` entries to your host `/etc/hosts` so your browser can
resolve them when OIDC redirects you to Kanidm. Without it, the login flow
from a host browser dead-ends at `kanidm.hearth.local` not resolving.

On first visit to `https://kanidm.hearth.local:8443` your browser will
warn about Kanidm's self-signed cert — accept it once or import
`dev/kanidm/cert.pem` into your browser's trust store.

## Test Accounts

| Username | Password | Role / Groups |
|---|---|---|
| `testadmin` | `test-demo-enrollment` | hearth-admins, hearth-users |
| `testoperator` | `test-demo-enrollment` | hearth-operators, hearth-users |
| `testviewer` | `test-demo-enrollment` | hearth-viewers, hearth-users |
| `testdev` | `test-demo-enrollment` | hearth-developers, hearth-users |
| `testdesigner` | `test-demo-enrollment` | hearth-designers, hearth-users |
| `testuser` | `test-demo-enrollment` | hearth-users |

## Service URLs

| Service       | From host                 | From enrolled VM                |
|---------------|---------------------------|---------------------------------|
| Hearth Web UI | http://localhost:3000     | https://api.hearth.local/       |
| Kanidm (IdP)  | https://localhost:8443    | https://kanidm.hearth.local/    |
| Element Chat  | http://localhost:8088     | https://chat.hearth.local/      |
| Nextcloud     | http://localhost:8089     | https://cloud.hearth.local/     |
| Grafana       | http://localhost:3001     | https://grafana.hearth.local/   |
| Attic (cache) | http://localhost:8080     | https://cache.hearth.local/     |
| Loki (logs)   | http://localhost:3100     | —                               |
| Headscale     | http://localhost:8085     | —                               |

> **In-VM URLs** are served by a local Caddy reverse proxy that terminates TLS
> on the host using a dev-only CA ("Hearth Dev CA") stored at
> `dev/caddy/root.crt`. Enrolled VMs auto-trust this CA because the NixOS
> enrollment module bakes it into the system trust store during `nix build`.
>
> If you want to reach the `*.hearth.local` URLs from your **host** browser
> too, add entries to your host `/etc/hosts` pointing each name to `127.0.0.1`
> and import `dev/caddy/root.crt` into your browser's trust store. This is
> optional — host-port URLs already work.
>
> **Kanidm direct URL** (`https://localhost:8443`) still uses Kanidm's own
> self-signed cert and will prompt for a browser warning on first visit.

## Demo Scenarios

### 1. Admin Dashboard (2 min)

1. Open http://localhost:3000
2. Click "Sign in" — redirects to Kanidm
3. Login as `testadmin` / `test-demo-enrollment`
4. Dashboard shows the live fleet (empty until you enroll something — boot
   `just fleet-vm` or `just enroll <name>` first)
5. Click **Machines** — enrolled devices appear here with status and role
6. Click a machine — see hardware report, heartbeat history, deployment status

### 2. Software Catalog (2 min)

1. Navigate to `/catalog`
2. Browse 18 software entries across 8 categories (Browser, Development, Design, etc.)
3. Use search to find "VS Code"
4. Click to see detail panel with install method and approval requirements
5. Click **Request** — creates a pending software request (requires an enrolled machine)
6. Switch to `/requests` (admin view) — see the new request

### 3. Deployment Pipeline (2 min)

1. Navigate to `/deployments`
2. The list is empty until you create a deployment against enrolled machines —
   real deployments are the only ones that show up here
3. Once you have an enrolled VM, create a deployment to see per-machine status,
   rollback reasons, and error messages

### 4. Compliance (1 min)

1. Navigate to `/compliance`
2. See 4 compliance policies (firewall, SSH, auto-updates, FDE)
3. Policy results populate as deployments run against enrolled machines

### 5. Live Fleet VM (3 min)

1. In a second terminal:
   ```bash
   nix develop -c just fleet-vm
   ```
2. VM boots with GNOME desktop in ~60s
3. Back in web UI — refresh `/machines`
4. `hearth-fleet-vm` shows a recent heartbeat (green indicator)
5. In the VM terminal: `journalctl -fu hearth-agent` to see polling logs

### 6. Enrollment Flow (3 min)

1. In a second terminal:
   ```bash
   nix develop -c just enroll demo
   ```
2. TUI enrollment wizard appears
3. Walk through: Welcome, Hardware Detection, Network, Login, Enroll
4. Back in web UI — a new machine appears as "pending" in the machines list

### 6b. Enroll and use a VM end-to-end (10 min)

The star of the demo: enroll a fresh VM, reboot into the installed system, log
in as a user, and click through every service by clean hostname.

1. Make sure `just demo` is running in terminal 1.
2. In terminal 2, enroll a new VM named `demo`:
   ```bash
   nix develop -c just enroll demo
   ```
   This boots the enrollment ISO, which installs NixOS to
   `dev/vms/demo.qcow2`. Walk through the TUI (Hardware → Network → Login →
   Enroll). When install finishes, the VM powers off.
3. Boot the installed system:
   ```bash
   nix develop -c just start-vm demo
   ```
   GNOME + the Hearth greeter come up.
4. Log in as **`testuser` / `test-demo-enrollment`**.
5. Inside the VM, open the browser and hit each service by its clean URL — no
   port, no cert warnings:
   - **Catalog** → https://api.hearth.local/ (browse software, click Request)
   - **Kanidm** → https://kanidm.hearth.local/ (view your own account)
   - **Element** → https://chat.hearth.local/ (join `#general`)
   - **Nextcloud** → https://cloud.hearth.local/ (browse Documents)
   - **Grafana** → https://grafana.hearth.local/ (open "Hearth Fleet Overview")
6. Back on the host, switch to terminal 1's web UI: `testuser`'s VM now shows
   a recent heartbeat in `/machines`.
7. Manage VMs:
   ```bash
   nix develop -c just list-vms          # see all enrolled VMs
   nix develop -c just enroll alice      # spin up a second one
   nix develop -c just destroy-vm alice  # nuke alice's disk
   ```

### 7. Platform Services (2 min)

1. Navigate to `/services` in the web UI
2. See integrated services: Chat, Cloud Storage, Identity
3. Open **Element Chat** at http://localhost:8088
4. Login with `testadmin` credentials
5. See pre-created rooms: #general, #random, #it-support
6. Open **Nextcloud** at http://localhost:8089
7. See default folders: Documents, Projects, Shared

### 8. Observability (1 min)

1. Open **Grafana** at http://localhost:3001 (no login needed — anonymous admin)
2. Navigate to "Hearth Fleet Overview" dashboard
3. Loki datasource is pre-configured for log queries

## Re-seeding Data

To re-seed without re-running the full setup:

```bash
just seed
```

The seed script is idempotent — running it multiple times won't create duplicates.

## Tearing Down

```bash
just infra-down          # Stop all containers (data preserved in volumes)
docker volume prune      # Remove data volumes (destructive — starts fresh)
```
