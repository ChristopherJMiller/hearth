---
description: Perform STRIDE threat analysis on a Hearth component. Identifies trust boundaries, data flows, and attack paths. Rates threats by likelihood × impact and maps mitigations to NIST 800-53 controls.
argument-hint: "<component>"
---

Run a threat model analysis on the component specified in `$ARGUMENTS`.

## Components

Valid component names (normalize abbreviations and synonyms):

- `api` — the control plane REST API (`crates/hearth-api/`)
- `agent` — on-device agent (`crates/hearth-agent/`)
- `build-pipeline` / `build` — build worker + Attic cache path
  (`crates/hearth-build-worker/`)
- `enrollment` — device enrollment flow
  (`crates/hearth-api/src/routes/enrollment.rs` +
  `crates/hearth-enrollment/`)
- `identity` — Kanidm integration, OIDC flow, identity sync
- `mesh` — Headscale WireGuard mesh configuration
- `fleet-device` / `device` — the NixOS workstation itself (hardening,
  FDE, PAM, greeter)
- `web-frontend` / `web` — the React SPA (`web/apps/hearth/`)
- `helm-chart` / `helm` — Kubernetes deployment
  (`chart/hearth-home/`)
- `full` — entire system (large, use sparingly)

If the argument is missing or invalid, list the valid components and ask
the user to pick one.

## Execution

Spawn the `threat-model` subagent with a prompt that:

1. Identifies the component from `$ARGUMENTS`
2. Provides the list of relevant files for that component
3. Asks the subagent to read those files (not just skim) and produce
   the STRIDE analysis per its system prompt
4. Reminds it to reference `docs/threat-model.md` for existing accepted
   risks so it does not re-flag them

## Presentation

Show the user:
1. The list of trust boundaries and data flows identified
2. All threats grouped by severity (Critical → Low)
3. The top 3 threats with a one-line summary each
4. Quick wins the team could implement immediately
5. Any "open questions" the subagent surfaced — these often indicate
   design decisions the team hasn't explicitly made

Offer to:
- Append new findings to `docs/threat-model.md` under the relevant
  component section
- Open follow-up work items for High/Critical threats (as TODO
  comments in the relevant source file, or as a checklist in the PR
  description)

## Guidance

The component argument is critical — a threat model for the wrong
component will produce misleading results. If you're unsure which
component the user means, ask before running the agent.
