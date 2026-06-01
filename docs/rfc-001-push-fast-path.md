# RFC-001: Control-plane → agent push fast-path

**Status:** Draft
**Owner:** TBD
**Tracking:** ROADMAP.md → Operational Backlog → "Push mechanism: control-plane → agent fast-path"

## Problem

Every closure change today reaches a fleet device on a 60–120s round trip:

1. Operator (or self-service `PUT /api/v1/me/config`) edits a per-user closure.
2. Worker claims the job, evaluates the flake tarball, builds, signs, pushes to Attic.
3. `complete_user_env_build` fans out `target_closure` to all `user_environments` for the user (`crates/hearth-api/src/repo.rs:1759`).
4. Agent polls `/api/v1/machines/{id}/state` every `poll_interval_secs` (default **60s** — `crates/hearth-common/src/config.rs:155`).
5. Agent realises a new closure, pulls from Attic, activates.

The 60s poll is the dominant latency for any change small enough to land on a warm cache. Each hop also hides a class of bug — the dev loop has burned cycles on cache-pubkey rotation, attic substituter mismatch, worker DB stalls, and "did this even reach the queue?". A fast-path that lets us land a closure on a device in seconds (without ripping out the durable pipeline) would shorten both the dev loop and the user-perceived "I changed my settings" → "I see the change" interval.

## Goals

- **Dev push:** sub-5s "host → running fleet VM" closure swap for the iterating contributor. One command, no worker, no Attic push, no DB write.
- **Prod push:** sub-2s "control-plane completes build" → "agent begins activation" for any online device, falling back cleanly to polling.
- **Auth and audit unchanged.** Machine tokens still gate every transition. Fan-out still goes through `user_environments` so an offline device picks up the change at next poll. Audit events still fire from the API path.
- **No new content-delivery system.** Attic remains the durable closure store for prod; SSH/9p remains the dev shortcut. We are notifying agents earlier, not replacing distribution.

## Non-goals

- Replacing Attic for content delivery.
- Removing the build-worker queue.
- A new agent → control-plane data channel (this is one-way: control plane notifies, agent fetches).
- Push from agent to agent (mesh distribution is a separate roadmap item).

## Design

### Dev push (`just push-user-env <user>`)

The fleet VM (`dev/fleet-vm.nix`) already exposes SSH on `host:2222` and runs the agent IPC at `/run/hearth/agent.sock`. Sequence:

1. Host runs `nix build .#userEnv.<user>` (re-using `lib.buildUserEnv`).
2. Host runs `nix copy --to ssh-ng://root@localhost:2222\?ssh-key=dev/fleet-vm-ssh-key <closure>`. The signed dev key already exists from `just setup`.
3. Host sends a new IPC request `ApplyClosure { username, closure }` over the agent socket via `ssh root@localhost:2222 hearth-cli push-closure …`. The agent runs the same activation path as the polled flow (`updater::apply_user_closure`).
4. Skipped on this path: worker queue, Attic push, DB rows, fan-out, poll. The DB stays consistent at next worker run or `just push-cache`.

Adds:
- `AgentRequest::ApplyClosure { username: String, closure: String }` in `crates/hearth-common/src/ipc.rs` (matches the existing tagged-enum pattern, no new transport).
- A `hearth-cli push-closure` thin client (or extend `hearth-agentctl` if introduced).
- `just push-user-env <user>` recipe and `just push-machine-config` sibling for system closures.

Risk: a host-pushed closure can drift from what the worker would have built. Mitigation: gate the IPC handler on a build-time `cfg(hearth_dev)` flag, refuse on production tokens, and stamp the closure path into `/var/lib/hearth/last-dev-push` so the next regular sync overwrites it.

### Prod push (control-plane → agent notify)

Open a long-lived **server-sent events** stream from agent to API:

- `GET /api/v1/machines/{id}/events` (`MachineIdentity` auth, existing machine-token JWT).
- Server holds the connection and emits newline-delimited JSON events. One event today: `{"type":"state_changed"}`.
- Agent treats an event as "poll now" — same code path as the timer-driven loop. No new logic in the activation hot path.
- Heartbeat: API sends a `:keepalive` comment every 30s. Agent reconnects on timeout/EOF with capped exponential backoff (1s → 30s).
- Offline-tolerant: when the stream is down, the existing `poll_interval_secs` poll continues. The push is an optimisation, never a correctness primitive.

Why SSE, not WebSocket:
- One-way control-plane → agent traffic is the whole requirement. SSE is HTTP/1.1, traverses every load balancer Hearth already supports, and survives existing Caddy/oauth2-proxy configs without sticky-session tuning.
- Resumable via `Last-Event-ID` if we later add per-machine event IDs; not needed for "poll now" semantics.

Fan-out on the server side:
- An in-process `tokio::broadcast::Sender<MachineEvent>` keyed by `machine_id`.
- `complete_user_env_build` (and the equivalent system-closure path) looks up affected machine IDs from `user_environments` and publishes to each broadcast slot.
- Multi-replica API: each replica owns its subscribers; cross-replica fan-out goes through Postgres `LISTEN/NOTIFY` (one notify, every replica forwards to its local subscribers). Same pattern Synapse and similar tools use; avoids dragging in Redis.

### Cadence and back-pressure

- Default poll interval drops from 60s to **300s (5 min)** once a device has a healthy event stream, and snaps back to 60s on disconnect. Net: fewer DB hits, faster perceived updates, no behaviour change for offline-only devices.
- If an agent receives more than N events/min (suggested N=10), it coalesces — one poll per debounce window — so a noisy fan-out can't DoS itself.

## Open questions

1. **CLI ergonomics.** `just push-user-env` is a recipe wrapping a `hearth-cli` subcommand. Do we want the subcommand published as a separate binary, or rolled into `hearth-agentctl` once that exists?
2. **Event payload.** Does v1 ship `{"type":"state_changed"}` only, or include `target_closure` so the agent can skip the `/state` round trip? Including it doubles the payload size but cuts one HTTP request per event.
3. **Auth for SSE through oauth2-proxy.** The existing machine token is a bearer JWT; oauth2-proxy passes it through. Verify Caddy doesn't strip long-lived `Transfer-Encoding: chunked` responses with the default config.
4. **Cross-replica fan-out — `LISTEN/NOTIFY` or something else?** Adequate for the expected fleet size (<10k devices, <10 API replicas). Document the upper bound and a migration path (Redis pub/sub) for larger deployments.

## Rollout

1. ~~RFC accepted (this doc).~~
2. ~~Land `AgentRequest::ApplyClosure` + `just push-user-env` behind the dev-only IPC gate. Zero impact on prod.~~ **Done.** Variant lives in `crates/hearth-common/src/ipc.rs`; handler in `crates/hearth-agent/src/ipc.rs::handle_apply_closure` gated by the `HEARTH_ENABLE_DEV_PUSH=1` env var (set in `dev/fleet-vm.nix`, unset in production). Wire-format tests in `crates/hearth-common/src/ipc.rs::tests`. CLI in the `push-user-env` justfile recipe (Python AF_UNIX client over the existing SSH:2222 fleet-VM shortcut).
3. Add the SSE endpoint + broadcast plumbing on the API. Agent opts in via config (`push.enabled = false` initially).
4. Flip `push.enabled = true` in the default agent config once observed reconnect-storm metrics look sane on the dev fleet.
5. Drop poll cadence to 300s under healthy stream; ship.

## Out of scope (deliberate)

- Replacing Attic for closure distribution.
- Replacing the build worker queue.
- Agent → agent peer distribution.
- Compaction of `user_env_build_jobs` (separate operational concern).
