# Hearth Demo Walkthrough

## Prerequisites

- Docker and Docker Compose
- Nix with flakes enabled
- ~10 GB free disk space, ~8 GB RAM for all services

## Quick Start

```bash
nix develop
just demo
```

Wait ~3 minutes for all services to start and initialize. The API server starts last and prints service URLs.

## Test Accounts

| Username | Password | Role / Groups |
|---|---|---|
| `testadmin` | `test-demo-enrollment` | hearth-admins, hearth-users |
| `testdev` | `test-demo-enrollment` | hearth-developers, hearth-users |
| `testdesigner` | `test-demo-enrollment` | hearth-designers, hearth-users |
| `testuser` | `test-demo-enrollment` | hearth-users |

## Service URLs

| Service | URL |
|---|---|
| Hearth Web UI | http://localhost:3000 |
| Kanidm (IdP) | https://localhost:8443 |
| Element Chat | http://localhost:8088 |
| Nextcloud | http://localhost:8089 |
| Grafana | http://localhost:3001 |
| Loki (logs) | http://localhost:3100 |
| Attic (cache) | http://localhost:8080 |
| Headscale | http://localhost:8085 |

> **Note:** Kanidm uses a self-signed TLS certificate. Accept the browser warning on first visit.

## Demo Scenarios

### 1. Admin Dashboard (2 min)

1. Open http://localhost:3000
2. Click "Sign in" — redirects to Kanidm
3. Login as `testadmin` / `test-demo-enrollment`
4. Dashboard shows: 8 machines, 5 active, 1 pending enrollment, active deployments
5. Click **Machines** — see fleet with varied statuses and roles
6. Click a machine — see hardware report, heartbeat history, deployment status

### 2. Software Catalog (2 min)

1. Navigate to `/catalog`
2. Browse 18 software entries across 8 categories (Browser, Development, Design, etc.)
3. Use search to find "VS Code"
4. Click to see detail panel with install method and approval requirements
5. Click **Request** — creates a pending software request
6. Switch to `/requests` (admin view) — see the new request alongside existing ones

### 3. Deployment Pipeline (2 min)

1. Navigate to `/deployments`
2. See 4 deployments in various states (completed, rolling, canary, failed)
3. Click the **rolling** deployment — see per-machine status (1 completed, 1 downloading, 1 pending)
4. Click the **failed** deployment — see rollback reason and per-machine error messages

### 4. Compliance (1 min)

1. Navigate to `/compliance`
2. See 4 compliance policies (firewall, SSH, auto-updates, FDE)
3. Click the completed deployment — see per-machine policy results (some passing, some failing)

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
   nix develop -c just enroll
   ```
2. TUI enrollment wizard appears
3. Walk through: Welcome, Hardware Detection, Network, Login, Enroll
4. Back in web UI — a new machine appears as "pending" in the machines list

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
