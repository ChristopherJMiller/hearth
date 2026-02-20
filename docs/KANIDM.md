# Kanidm Integration

Hearth uses [Kanidm](https://kanidm.github.io/kanidm/) as its identity provider. This document covers the dev setup, REST API patterns, and how Kanidm integrates with the platform.

## Dev Setup

### Prerequisites

```bash
docker compose up -d kanidm   # Start the Kanidm 1.9 container
bash dev/kanidm/bootstrap.sh  # Provision groups, users, OAuth2 clients
source dev/kanidm/.env         # Load credentials into shell
```

The bootstrap script is idempotent — safe to re-run. It generates a self-signed TLS cert on first run (covers `localhost` and `10.0.2.2` for QEMU VMs).

### Container Configuration

| Setting | Value |
|---------|-------|
| Image | `kanidm/server:1.9.0` |
| Port | `8443` (HTTPS only) |
| TLS | Self-signed cert at `dev/kanidm/cert.pem` |
| Config | `dev/kanidm/server.toml` |
| Data | `kanidm-data` Docker volume |

### Admin Accounts

Kanidm has two built-in admin accounts with different roles:

| Account | Purpose | Use For |
|---------|---------|---------|
| `admin` | System administrator | OAuth2 clients, service accounts, system config |
| `idm_admin` | Identity manager | Groups, persons, credential management |

Passwords are recovered via `kanidmd recover-account <name>` inside the container. Each recovery generates a new random password.

### Test Users

The bootstrap creates four test users covering all role groups. Passwords are randomly generated on each bootstrap run — check `dev/kanidm/.env` for current values.

| User | Groups |
|------|--------|
| `testadmin` | hearth-admins, hearth-users |
| `testdev` | hearth-developers, hearth-users |
| `testdesigner` | hearth-designers, hearth-users |
| `testuser` | hearth-users |

### OAuth2 Clients

| Client ID | Flow | Used By |
|-----------|------|---------|
| `hearth-console` | Authorization Code + PKCE | Admin console SPA |
| `hearth-enrollment` | Device Authorization Grant | Enrollment TUI |

## REST API Reference

Kanidm exposes a JSON REST API over HTTPS. All examples below use `curl -sk` (silent, insecure for self-signed certs).

### Authentication

Kanidm uses a multi-step authentication flow. Each step returns session state, and subsequent steps must include the session identifier.

#### Step 1: Init

```bash
curl -sk -D /tmp/headers -X POST "$KANIDM_URL/v1/auth" \
  -H "Content-Type: application/json" \
  -d '{"step":{"init":"admin"}}'
```

Response:
```json
{"sessionid":"...","state":{"choose":["password"]}}
```

Extract the session ID from the `X-KANIDM-AUTH-SESSION-ID` response header.

#### Step 2: Begin method

```bash
curl -sk -X POST "$KANIDM_URL/v1/auth" \
  -H "Content-Type: application/json" \
  -H "X-KANIDM-AUTH-SESSION-ID: $SESSION" \
  -d '{"step":{"begin":"password"}}'
```

#### Step 3: Provide credentials

```bash
curl -sk -X POST "$KANIDM_URL/v1/auth" \
  -H "Content-Type: application/json" \
  -H "X-KANIDM-AUTH-SESSION-ID: $SESSION" \
  -d '{"step":{"cred":{"password":"the-password"}}}'
```

Response on success:
```json
{"sessionid":"...","state":{"success":"eyJhbG..."}}
```

The `state.success` value is a JWT bearer token for subsequent API calls.

#### Using the token

```bash
curl -sk "$KANIDM_URL/v1/self" \
  -H "Authorization: Bearer $TOKEN"
```

### Groups

**Create:**
```bash
POST /v1/group
{"attrs":{"name":["my-group"]}}
```

**Get:**
```bash
GET /v1/group/{name}
```

Returns `null` with HTTP 200 if the group doesn't exist — check the response body, not just the status code.

**Add member:**
```bash
POST /v1/group/{name}/_attr/member
["username"]
```

### Persons

**Create:**
```bash
POST /v1/person
{"attrs":{"name":["jdoe"],"displayname":["Jane Doe"]}}
```

**Get:**
```bash
GET /v1/person/{name}
```

### Credential Updates

Kanidm uses a session-based credential update flow. You open a session, make changes, then commit. The session token is included in the request body (not as a header).

#### Begin session (authenticated admin)

```bash
GET /v1/person/{name}/_credential/_update
Authorization: Bearer $IDM_TOKEN
```

Response: a 2-element array `[CUSessionToken, CUStatus]`
```json
[
  {"token":"session-token-here"},
  {"can_commit":false,"warnings":["NoValidCredentials"],"primary_state":"Modifiable",...}
]
```

#### Set password

```bash
POST /v1/credential/_update
[{"password":"the-new-password"}, {"token":"session-token-here"}]
```

The payload is a 2-element JSON array: `[CURequest, CUSessionToken]`.

Response: `CUStatus` with updated `can_commit` and `warnings`.

#### Commit

```bash
POST /v1/credential/_commit
{"token":"session-token-here"}
```

Returns `null` on success.

#### Alternative: Intent token flow

For credential resets (e.g. sending a reset link), the intent token flow allows the exchange step to be unauthenticated:

1. `GET /v1/person/{name}/_credential/_update_intent` (authenticated) — returns `{"token":"short-token","expiry_time":...}`
2. `POST /v1/credential/_exchange_intent` with body `"short-token"` (no auth) — returns `[CUSessionToken, CUStatus]`
3. `POST /v1/credential/_update` and `POST /v1/credential/_commit` as above (no auth, session token in body)

#### CURequest variants

The first element of the update payload is a `CURequest` enum (serde `rename_all = "lowercase"`):

| Variant | JSON | Description |
|---------|------|-------------|
| Password | `{"password":"..."}` | Set primary password |
| UnixPassword | `{"unixpassword":"..."}` | Set POSIX password |
| TotpGenerate | `{"totpgenerate":null}` | Start TOTP enrollment |
| TotpVerify | `{"totpverify":[123456,"label"]}` | Verify TOTP code |
| PasskeyInit | `{"passkeyinit":null}` | Start passkey enrollment |
| PrimaryRemove | `{"primaryremove":null}` | Remove primary credential |
| SshPublicKey | `{"sshpublickey":["label","ssh-ed25519 ..."]}` | Add SSH key |

### Service Accounts

**Create:**
```bash
POST /v1/service_account
{"attrs":{"name":["my-svc"],"displayname":["My Service"]}}
```

**Generate API token:**
```bash
POST /v1/service_account/{name}/_api_token
{"label":"token-label","expiry":null}
```

Returns the token string directly.

### OAuth2 Resource Servers

**Create:**
```bash
POST /v1/system/oauth2
{"attrs":{"oauth2_rs_name":["my-client"],"displayname":["My Client"],"oauth2_rs_origin":["https://example.com"]}}
```

**Get:**
```bash
GET /v1/system/oauth2/{name}
```

**Set scope map:**
```bash
POST /v1/system/oauth2/{name}/_scopemap/{group}
["openid","profile","email","groups"]
```

**Configure settings (e.g. localhost redirects):**
```bash
PATCH /v1/system/oauth2/{name}
{"attrs":{"oauth2_allow_localhost_redirect":["true"]}}
```

### Account Policies

Kanidm controls authentication requirements through account policies on groups. The key attribute is `credential_type_minimum`.

| Value | Meaning |
|-------|---------|
| `any` | Password-only allowed |
| `mfa` | MFA required (password + TOTP/passkey) |
| `passkey` | Passkey required |
| `attested_passkey` | Attested passkey required |

Policy resolution takes the **most restrictive** value across all groups a user belongs to.

**Enable account policy on a group:**
```bash
POST /v1/group/{name}/_attr/class
["account_policy"]
```

**Set credential minimum:**
```bash
PUT /v1/group/{name}/_attr/credential_type_minimum
["any"]
```

> **Note:** Builtin groups (like `idm_all_persons`) cannot be modified by `admin`. The bootstrap sets `credential_type_minimum=any` where possible for dev convenience.

## Platform Integration Points

### Enrollment TUI (`hearth-enrollment`)

Uses the **OAuth2 Device Authorization Grant** (RFC 8628):

1. `POST /oauth2/device` with `client_id=hearth-enrollment` — starts the device flow
2. Displays verification URL + QR code to the operator
3. Polls `POST /oauth2/token` with the device code until the user authenticates

The Kanidm URL is injected via `HEARTH_KANIDM_URL` env var (set by the NixOS enrollment module from `flake.nix`).

### Admin Console (`@hearth/catalog`)

Uses **Authorization Code + PKCE**:

- Authority: `$KANIDM_URL/oauth2/openid/hearth-console`
- Client ID: `hearth-console`
- Scopes: `openid profile email groups`

Configured in `web/apps/console/src/auth.ts`.

### Agent (`hearth-agent`)

Does not directly authenticate with Kanidm. The API server validates tokens from the console/enrollment and communicates fleet state to agents over the agent API.

### NixOS Modules

- `modules/kanidm-client.nix` — Configures Kanidm client on fleet machines (URI, CA cert)
- `modules/enrollment.nix` — Injects `HEARTH_KANIDM_URL` and `HEARTH_KANIDM_CLIENT_ID` for the enrollment TUI
- `lib/mk-enrollment-image.nix` — Accepts `kanidmUrl` parameter for ISO builds
- `lib/mk-fleet-host.nix` — Accepts `kanidmUrl` for fleet machine configs

## Troubleshooting

### "error sending request for url" in enrollment VM

The Kanidm container must be running and reachable from the VM at `https://10.0.2.2:8443` (QEMU user-mode networking gateway). Check:

```bash
docker compose up -d kanidm
curl -sk https://localhost:8443/status  # should return: true
```

### "TLS Private Key and Certificate Chain are required"

The self-signed cert is missing. Run the bootstrap to generate it:

```bash
bash dev/kanidm/bootstrap.sh
```

Or generate manually:

```bash
openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
  -keyout dev/kanidm/key.pem -out dev/kanidm/cert.pem \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:10.0.2.2,IP:::1"
```

### "invalid credential state" on login

The user account has no credentials set. Re-run the bootstrap or recover the account manually:

```bash
docker exec hearth-kanidm-1 kanidmd recover-account <username>
```

### CorruptedEntry / DB incompatibility

The Kanidm data volume was created by a different server version. Wipe and recreate:

```bash
docker compose down kanidm
docker volume rm hearth_kanidm-data
docker compose up -d kanidm
bash dev/kanidm/bootstrap.sh
```
