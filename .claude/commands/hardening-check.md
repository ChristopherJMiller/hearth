---
description: Audit infrastructure configs (Helm chart, NixOS modules, docker-compose) for hardening gaps. Produces prioritized findings with specific fix code (YAML/Nix snippets).
argument-hint: "[helm|nixos|docker-compose|all]"
---

Run the `hardening-reviewer` agent on infrastructure configurations.

## Target scope

Parse `$ARGUMENTS` for the scope:

- `helm` (default) — review `chart/hearth-home/` templates and values
- `nixos` — review `modules/*.nix` and `home-modules/*.nix`
- `docker-compose` — review `infra/docker-compose.yml` or
  `docker-compose.yml` (dev-only)
- `all` — all three scopes in one pass (slower but comprehensive)

If no argument is given, default to `helm` (the biggest gap area).

## Execution

Spawn the `hardening-reviewer` subagent with a prompt that:

1. States the target scope from `$ARGUMENTS`
2. For **helm**: ask it to read every Deployment, StatefulSet, DaemonSet,
   Job, and Service template under `chart/hearth-home/templates/`,
   along with `values.yaml`, and check against the Kubernetes Pod
   Security Standards (`restricted` profile) and CIS Kubernetes benchmark
3. For **nixos**: ask it to read `modules/hardening.nix` and any
   changed modules, plus `modules/compliance/*.nix`, and check against
   CIS Linux workstation benchmark sections 1, 3, 4, 5
4. For **docker-compose**: ask it to check non-root users, read-only
   root filesystems, network isolation, secret handling
5. For **all**: run all three in one report, grouped by scope

Instruct the subagent to produce fix code (YAML for Helm,
docker-compose; Nix for modules) matching the repo's existing style and
templating patterns.

## Presentation

Show the user:
1. A one-line summary per scope (e.g., "Helm: 12 findings — 3 Critical,
   5 High, 4 Medium")
2. All Critical + High findings with their fix snippets inline
3. Medium and Low findings as a collapsible list
4. A "quick wins" section the user can accept in bulk

Offer to:
- Apply the fix code directly to the affected files
- Create new files if the fix requires them (e.g., a new
  `NetworkPolicy` template)
- Run `helm lint`, `helm template`, and `helm-unittest` to verify the
  chart still renders after changes
- Run `just helm-security` (if it exists) to re-measure coverage

## Verification after fixes

If the user accepts fixes for the helm scope, remind them to run:
```
just helm-check
```
which runs `helm lint`, unit tests, and `kubeconform` schema validation.
