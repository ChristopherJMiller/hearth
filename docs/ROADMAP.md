# Roadmap

Living list of things we know we want next. Items in **Near-term** are
sized for an upcoming PR or two; **Architectural** items are larger
designs we want to land but need an RFC or refactor. Enterprise gap
items (printing, backup, asset lifecycle, etc.) live in
`pieces-to-fill-in.txt`.

## Near-term

- **Universal Kanidm SSO for every bundled service.** Today only
  Synapse force-redirects to Kanidm; Nextcloud has `user_oidc` set up
  as a *secondary* provider but its desktop client still hits the
  built-in webflow login. Stalwart and Grafana need verifying. Bake
  "Kanidm is the only login surface" into every service we ship — make
  user_oidc the default provider, hide the local-login form, and
  validate the desktop-client OIDC flow end-to-end. Tracked in
  `dev/nextcloud/bootstrap.sh` + Helm chart service values.

- **`config_hash` must include `fleet_config` fingerprint.** Per-user
  closures bake URLs from `HEARTH_*_URL`, but `config_hash` is
  computed from `username + role + overrides` only. Changing
  `HEARTH_CLOUD_URL` doesn't trigger a rebuild — we had to manually
  null `latest_closure` during today's URL refactor. Fix in
  `crates/hearth-api/src/repo.rs:upsert_user_config`. See
  `memory/feedback_user_env_hash_misses_fleet_config.md` for context.

- **System-closure dev-push fast-path.** RFC-001 covers per-user
  closures. The agent itself + the rest of the system closure still
  require a full `nix run .#fleet-vm` rebuild + reboot per change.
  Add a sibling of `push-user-env` that pushes a built
  `system.build.toplevel` and triggers
  `switch-to-configuration switch` in the VM. Gated by
  `HEARTH_ENABLE_DEV_PUSH=1` like the user-env path.

- **Fix `push-user-env` IPC quoting** (done as of today's PR — flagging
  as an example of the broader pattern: shell-escape bugs in justfile
  recipes that route through `just fleet-exec` need a `nix run` or
  similar wrapper instead of nested quoting).

- **GUI baseline UX audit.** Today `appindicator` was missing from the
  default role, so every Qt/Electron app's tray icon was invisible.
  Audit for similarly silent baseline-UX gaps: notification daemon,
  XDG desktop portal, clipboard manager, screen-share permissions.

## Architectural

- **Configurable service domain.** `*.hearth.local` is a dev fixture.
  Real orgs deploying Hearth want `cloud.acme.corp`, not Hearth
  branding in their URLs. Make the domain a single deploy-time
  variable propagated through:
    - Helm chart values (`global.domain`)
    - Caddy / ingress TLS SANs and ACME hostnames
    - Kanidm origin + OIDC client redirect URIs
    - Agent enrollment payload baked into `agent.toml`
    - `dev/services.env` (override for local dev)
    - `dev/fleet-vm.nix` `networking.hosts`
  Default: `hearth.local` for dev, prompt at Helm install for prod.

- **Control-plane DNS for fleet services.** Fleet VMs currently learn
  `*.hearth.local` from `networking.hosts` baked at enrollment time.
  That breaks the moment a service moves IPs or the cluster
  reorganizes. Right answer: serve a fleet-internal zone from the
  control plane (CoreDNS in the Helm chart, plus a `dnsmasq` /
  systemd-resolved override on enrolled nodes). Tie into the
  configurable-domain item above.

- **Per-machine fleet config in the DB.** Worker reads `HEARTH_*_URL`
  from env at build time and bakes them into the closure. That means
  every user-env closure is implicitly tied to "whatever env this
  worker happened to have." For multi-site / multi-fleet support, the
  service URLs belong on the `machines` row (or a child
  `machine_fleet_config` table) and the worker should look them up
  per build job. Removes env-smuggling, makes closures actually
  machine-specific, and unlocks the configurable-domain story above.

- **Machine-agnostic closures via runtime templating.** Going further
  than the previous item: ship closures with template placeholders
  (`@SERVER_URL@` etc.), let the agent fill them in at activate time
  from `agent.toml`. Same closure works for every machine in the
  fleet → much better cache hit rate (one closure per
  `(user, role, overrides)` instead of per-machine). Bigger refactor:
  every `home-modules/*.nix` that writes a URL into a file needs
  templating support, and the activate wrapper grows non-trivially.

- **Surface specific failure modes** (partly done in today's PR).
  Extend the pattern beyond user-env activation: every place we
  return an error to a user — greeter, web console, OIDC flows — must
  distinguish failure *kinds*, not just "something went wrong." See
  `memory/feedback_error_surfacing.md`.

## Done today

For context (closed out 2026-05-31 / 2026-06-01):

- Kanidm 1.10 POSIX enablement (explicit `_unix` POST in bootstrap).
- Fleet-config tarball includes `cpp/` so home-manager LibreOffice
  builds find the C++ bridge source.
- Agent: per-user activate now overrides `USER`/`LOGNAME` to short
  name (closure expects it), sets `HOME_MANAGER_BACKUP_EXT=hm-bak`,
  captures stdout (not just stderr) for actionable error messages,
  parses home-manager activate stdout for progress events, fires
  `notify-send` after dev-push activation.
- `UserEnvClosureResponse.build_error` propagates worker failures to
  the greeter; agent classifies failure as build-failed /
  unreachable / timeout instead of one generic message.
- `home-modules/nextcloud.nix` installs `nextcloud.cfg` as a writable
  copy via `home.activation` (was a /nix/store symlink, blocked
  QSettings).
- `appindicator` baked into `home-modules/common.nix` (was only in
  `developer` role; everyone needs tray support).
- Nextcloud `trusted_domains` includes `cloud.hearth.local` so the
  fleet VM can connect.
- Single source of truth for dev service URLs in `dev/services.env`,
  sourced by `dev`, `dev-full`, `dev-watch`, `worker` recipes.
- Fleet VM gets `virtio-vga-gl` + `gl=on` so guest Mesa has a real
  DRM device (Qt/WebEngine apps stop crashing at Vulkan init).
- `push-user-env` justfile recipe rewritten to inline the Python over
  SSH heredoc instead of layered quoting through `just fleet-exec`.
