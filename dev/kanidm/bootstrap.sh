#!/usr/bin/env bash
# dev/kanidm/bootstrap.sh — Provision Kanidm with Hearth identity structure
#
# Idempotent: safe to run multiple times. Creates groups, service accounts,
# and OAuth2 clients needed by the Hearth platform.
#
# Uses the Kanidm REST API directly (via curl) — no kanidm CLI needed.
#
# Prerequisites:
#   - Kanidm container running (docker-compose up kanidm)
#   - curl and jq available
#
# Usage:
#   bash dev/kanidm/bootstrap.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KANIDM_URL="${KANIDM_URL:-https://localhost:8443}"
KANIDM_CONTAINER="${KANIDM_CONTAINER:-hearth-kanidm-1}"

# curl flags: silent, insecure (self-signed dev cert)
C="curl -sk"

declare -A USER_PASSWORDS

echo "==> Kanidm bootstrap for Hearth"
echo "    URL: $KANIDM_URL"

# ---------------------------------------------------------------------------
# Step 0: Generate self-signed TLS cert if missing
# ---------------------------------------------------------------------------
CERT_FILE="$SCRIPT_DIR/cert.pem"
KEY_FILE="$SCRIPT_DIR/key.pem"

if [ ! -f "$CERT_FILE" ] || [ ! -f "$KEY_FILE" ]; then
    echo ""
    echo "==> Generating self-signed TLS certificate..."
    openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
        -keyout "$KEY_FILE" \
        -out "$CERT_FILE" \
        -subj "/CN=kanidm.hearth.local" \
        -addext "subjectAltName=DNS:kanidm.hearth.local,DNS:localhost,IP:127.0.0.1,IP:10.0.2.2,IP:::1" \
        2>/dev/null
    echo "    Generated cert.pem and key.pem"
else
    echo "    TLS certificate already exists"
fi

# ---------------------------------------------------------------------------
# Step 1: Recover admin credentials
# ---------------------------------------------------------------------------
echo ""
echo "==> Recovering admin accounts..."

RECOVER_OUTPUT=$(docker exec "$KANIDM_CONTAINER" kanidmd recover-account admin 2>&1 || true)
ADMIN_PASS=$(echo "$RECOVER_OUTPUT" | grep -oP 'new_password:\s*"\K[^"]+' || true)

if [ -z "$ADMIN_PASS" ]; then
    echo "    Could not recover admin password. Kanidm may not be ready."
    echo "    Output: $RECOVER_OUTPUT"
    echo "    Try: docker-compose up -d kanidm && sleep 5 && re-run this script."
    exit 1
fi
echo "    admin password recovered"

IDM_RECOVER=$(docker exec "$KANIDM_CONTAINER" kanidmd recover-account idm_admin 2>&1 || true)
IDM_ADMIN_PASS=$(echo "$IDM_RECOVER" | grep -oP 'new_password:\s*"\K[^"]+' || true)

if [ -z "$IDM_ADMIN_PASS" ]; then
    echo "    Could not recover idm_admin password."
    exit 1
fi
echo "    idm_admin password recovered"

# ---------------------------------------------------------------------------
# Auth helper: authenticate via REST API, return bearer token
# ---------------------------------------------------------------------------
auth_kanidm() {
    local user="$1" pass="$2"
    local cookies headers

    cookies=$(mktemp)
    headers=$(mktemp)

    # Step 1: init (save cookies for session tracking)
    $C -D "$headers" -c "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d "$(jq -n --arg u "$user" '{"step":{"init":$u}}')" > /dev/null

    # Step 2: begin password method
    $C -b "$cookies" -c "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d '{"step":{"begin":"password"}}' > /dev/null

    # Step 3: provide password
    local auth_resp
    auth_resp=$($C -b "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d "$(jq -n --arg p "$pass" '{"step":{"cred":{"password":$p}}}')")

    rm -f "$headers" "$cookies"
    echo "$auth_resp" | jq -r '.state.success // empty'
}

echo ""
echo "==> Authenticating..."
ADMIN_TOKEN=$(auth_kanidm "admin" "$ADMIN_PASS")
IDM_TOKEN=$(auth_kanidm "idm_admin" "$IDM_ADMIN_PASS")

if [ -z "$ADMIN_TOKEN" ] || [ -z "$IDM_TOKEN" ]; then
    echo "    Authentication failed"
    exit 1
fi
echo "    Authenticated as admin and idm_admin"

# ---------------------------------------------------------------------------
# Dev credential policy: allow password-only auth (no MFA requirement)
# ---------------------------------------------------------------------------
echo ""
echo "==> Configuring dev credential policy..."

# Enable account_policy class on idm_all_persons
# These calls require idm_admin (identity management), not admin (system).
$C -X POST "$KANIDM_URL/v1/group/idm_all_persons/_attr/class" \
    -H "Authorization: Bearer $IDM_TOKEN" \
    -H "Content-Type: application/json" \
    -d '["account_policy"]' > /dev/null 2>&1 || true

# Set credential_type_minimum to "any" (allows password-only)
$C -X PUT "$KANIDM_URL/v1/group/idm_all_persons/_attr/credential_type_minimum" \
    -H "Authorization: Bearer $IDM_TOKEN" \
    -H "Content-Type: application/json" \
    -d '["any"]' > /dev/null 2>&1 || true

# Disable minimum password length for dev convenience
$C -X PUT "$KANIDM_URL/v1/group/idm_all_persons/_attr/auth_password_minimum_length" \
    -H "Authorization: Bearer $IDM_TOKEN" \
    -H "Content-Type: application/json" \
    -d '["1"]' > /dev/null 2>&1 || true

echo "    Set credential_type_minimum=any, auth_password_minimum_length=1 on idm_all_persons"

# ---------------------------------------------------------------------------
# REST API helpers
# ---------------------------------------------------------------------------
# admin token: system config (domain, service accounts)
# idm_admin token: groups, persons, credentials, OAuth2 (via idm_oauth2_admins)

idm_post() {
    $C -X POST "$KANIDM_URL$1" \
        -H "Authorization: Bearer $IDM_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2"
}

idm_patch() {
    $C -X PATCH "$KANIDM_URL$1" \
        -H "Authorization: Bearer $IDM_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2"
}

admin_post() {
    $C -X POST "$KANIDM_URL$1" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2"
}

# Check if resource exists (returns 0 if exists)
# Kanidm returns 200 with "null" body for missing resources, so check body too
resource_exists() {
    local body
    body=$($C "$KANIDM_URL$1" -H "Authorization: Bearer ${2:-$IDM_TOKEN}")
    [ "$body" != "null" ] && [ -n "$body" ] && [ "$body" != "\"notauthenticated\"" ] && [ "$body" != "\"accessdenied\"" ]
}

# Run a REST call and abort if it returns an error
checked_post() {
    local label="$1" response
    shift
    response=$(idm_post "$@")
    if echo "$response" | grep -qE '"(accessdenied|notauthenticated|invalidentrystate|schemaviolation)"'; then
        echo "    ERROR: $label failed: $response" >&2
        exit 1
    fi
}

checked_patch() {
    local label="$1" response
    shift
    response=$(idm_patch "$@")
    if echo "$response" | grep -qE '"(accessdenied|notauthenticated|invalidentrystate|schemaviolation)"'; then
        echo "    ERROR: $label failed: $response" >&2
        exit 1
    fi
}

create_group() {
    local name="$1"
    if resource_exists "/v1/group/$name"; then
        echo "    Group '$name' already exists"
    else
        checked_post "create group '$name'" "/v1/group" "{\"attrs\":{\"name\":[\"$name\"]}}"
        echo "    Created group '$name'"
    fi
}

add_group_member() {
    local group="$1" member="$2"
    idm_post "/v1/group/$group/_attr/member" "[\"$member\"]" > /dev/null 2>&1 || true
}

create_person() {
    local name="$1" display="$2"
    if resource_exists "/v1/person/$name"; then
        echo "    Person '$name' already exists"
    else
        checked_post "create person '$name'" "/v1/person" "{\"attrs\":{\"name\":[\"$name\"],\"displayname\":[\"$display\"]}}"
        echo "    Created person '$name'"
    fi
}

set_person_password() {
    local name="$1" desired_password="$2"

    # Step 1: Always recover-account first — this guarantees valid credentials
    # even if the REST API password change below fails.
    local recover_out fallback_pw
    recover_out=$(docker exec "$KANIDM_CONTAINER" kanidmd recover-account "$name" 2>&1)
    fallback_pw=$(echo "$recover_out" | grep -oP 'new_password:\s*"\K[^"]+' || true)
    if [ -z "$fallback_pw" ]; then
        echo "    Warning: could not set credentials for '$name'"
        return
    fi

    # Step 2: Try to change to the simple dev password via the credential
    # update REST API. If this fails, the recover-account password still works.
    local session_resp cu_token
    session_resp=$($C -X GET "$KANIDM_URL/v1/person/$name/_credential/_update" \
        -H "Authorization: Bearer $IDM_TOKEN")
    cu_token=$(echo "$session_resp" | jq -r '.[0].token // empty' 2>/dev/null)

    if [ -z "$cu_token" ]; then
        echo "    Set password for '$name' (random — credential update API unavailable)"
        USER_PASSWORDS["$name"]="$fallback_pw"
        return
    fi

    # Set the simple password
    $C -X POST "$KANIDM_URL/v1/credential/_update" \
        -H "Content-Type: application/json" \
        -d "[{\"password\":\"$desired_password\"},{\"token\":\"$cu_token\"}]" > /dev/null 2>&1

    # Commit
    local commit_resp
    commit_resp=$($C -X POST "$KANIDM_URL/v1/credential/_commit" \
        -H "Content-Type: application/json" \
        -d "{\"token\":\"$cu_token\"}")

    # If commit returned null, it succeeded
    if [ "$commit_resp" = "null" ] || [ -z "$commit_resp" ]; then
        echo "    Set password for '$name': $desired_password"
        USER_PASSWORDS["$name"]="$desired_password"
    else
        echo "    Set password for '$name' (random — simple password rejected)"
        USER_PASSWORDS["$name"]="$fallback_pw"
    fi
}

# Dev password — must pass Kanidm's zxcvbn Score::Four entropy check.
# A short passphrase satisfies the strength requirement while being easy to type.
DEV_PASSWORD="test-demo-enrollment"

# ---------------------------------------------------------------------------
# Step 2: Create groups
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating groups..."

create_group "hearth-users"
create_group "hearth-admins"
create_group "hearth-developers"
create_group "hearth-designers"

# hearth-admins should be a member of hearth-users
add_group_member "hearth-users" "hearth-admins"

# ---------------------------------------------------------------------------
# Step 3: Create test users
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating test users..."

create_person "testadmin" "Test Admin"
add_group_member "hearth-admins" "testadmin"
add_group_member "hearth-users" "testadmin"
set_person_password "testadmin" "$DEV_PASSWORD"

create_person "testdev" "Test Developer"
add_group_member "hearth-developers" "testdev"
add_group_member "hearth-users" "testdev"
set_person_password "testdev" "$DEV_PASSWORD"

create_person "testdesigner" "Test Designer"
add_group_member "hearth-designers" "testdesigner"
add_group_member "hearth-users" "testdesigner"
set_person_password "testdesigner" "$DEV_PASSWORD"

create_person "testuser" "Test User"
add_group_member "hearth-users" "testuser"
set_person_password "testuser" "$DEV_PASSWORD"

# ---------------------------------------------------------------------------
# Step 4: Create service account for API → Kanidm communication
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating service account..."

if resource_exists "/v1/service_account/hearth-api-svc" "$ADMIN_TOKEN"; then
    echo "    Service account 'hearth-api-svc' already exists"
else
    admin_post "/v1/service_account" \
        '{"attrs":{"name":["hearth-api-svc"],"displayname":["Hearth API service account"]}}' > /dev/null
    echo "    Created service account 'hearth-api-svc'"
fi

# Generate an API token
API_TOKEN=$($C -X POST "$KANIDM_URL/v1/service_account/hearth-api-svc/_api_token" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"label":"hearth-api","expiry":null}' 2>/dev/null | jq -r '. // empty' 2>/dev/null || echo "")

# ---------------------------------------------------------------------------
# Step 5: Register OAuth2 resource servers
# ---------------------------------------------------------------------------
echo ""
echo "==> Registering OAuth2 clients..."

# OAuth2 management requires idm_admin (member of idm_oauth2_admins), NOT admin.

# hearth-console: Public OAuth2 client (PKCE, no secret) for the web SPA
if resource_exists "/v1/oauth2/hearth-console" "$IDM_TOKEN"; then
    echo "    OAuth2 client 'hearth-console' already exists"
else
    checked_post "create OAuth2 client 'hearth-console'" "/v1/oauth2/_public" \
        '{"attrs":{"name":["hearth-console"],"displayname":["Hearth Admin Console"],"oauth2_rs_origin_landing":["http://localhost:5174"]}}'
    echo "    Created OAuth2 client 'hearth-console'"
fi

# Configure hearth-console
checked_post "scopemap hearth-console" "/v1/oauth2/hearth-console/_scopemap/hearth-users" \
    '["openid","profile","email","groups"]'

# Prefer short usernames + enable localhost redirect for dev
checked_patch "configure hearth-console" "/v1/oauth2/hearth-console" \
    '{"attrs":{"oauth2_prefer_short_username":["true"],"oauth2_allow_localhost_redirect":["true"]}}'

# hearth-enrollment: Public OAuth2 client (PKCE, no secret) for the enrollment kiosk browser
if resource_exists "/v1/oauth2/hearth-enrollment" "$IDM_TOKEN"; then
    echo "    OAuth2 client 'hearth-enrollment' already exists"
else
    checked_post "create OAuth2 client 'hearth-enrollment'" "/v1/oauth2/_public" \
        '{"attrs":{"name":["hearth-enrollment"],"displayname":["Hearth Device Enrollment"],"oauth2_rs_origin_landing":["https://kanidm.hearth.local:8443"]}}'
    echo "    Created OAuth2 client 'hearth-enrollment'"
fi

checked_post "scopemap hearth-enrollment" "/v1/oauth2/hearth-enrollment/_scopemap/hearth-users" \
    '["openid","profile","groups"]'

# Enable localhost redirects for enrollment kiosk browser
checked_patch "configure hearth-enrollment" "/v1/oauth2/hearth-enrollment" \
    '{"attrs":{"oauth2_allow_localhost_redirect":["true"],"oauth2_prefer_short_username":["true"]}}'

# ---------------------------------------------------------------------------
# Step 6: Write .env for local dev
# ---------------------------------------------------------------------------
echo ""
echo "==> Writing dev environment file..."

# hearth-console is a public client (PKCE) — no client secret
CONSOLE_SECRET="pkce-no-secret"

# Generate a stable machine token secret — reuse existing if present
if [ -f "$SCRIPT_DIR/.env" ]; then
    EXISTING_SECRET=$(grep '^HEARTH_MACHINE_TOKEN_SECRET=' "$SCRIPT_DIR/.env" 2>/dev/null | cut -d= -f2 || true)
fi
MACHINE_TOKEN_SECRET="${EXISTING_SECRET:-$(openssl rand -base64 32)}"

cat > "$SCRIPT_DIR/.env" <<EOF
# Generated by bootstrap.sh — do not commit
KANIDM_URL=$KANIDM_URL
KANIDM_ADMIN_PASSWORD=$ADMIN_PASS
KANIDM_IDM_ADMIN_PASSWORD=$IDM_ADMIN_PASS
KANIDM_OIDC_ISSUER=${KANIDM_URL}/oauth2/openid/hearth-console,${KANIDM_URL}/oauth2/openid/hearth-enrollment
KANIDM_OIDC_AUDIENCE=hearth-console,hearth-enrollment
KANIDM_ENROLLMENT_CLIENT_ID=hearth-enrollment
KANIDM_CONSOLE_CLIENT_ID=hearth-console
KANIDM_CONSOLE_SECRET=$CONSOLE_SECRET
HEARTH_API_SVC_TOKEN=$API_TOKEN
HEARTH_MACHINE_TOKEN_SECRET=$MACHINE_TOKEN_SECRET
TESTADMIN_PASSWORD=${USER_PASSWORDS[testadmin]:-}
TESTDEV_PASSWORD=${USER_PASSWORDS[testdev]:-}
TESTDESIGNER_PASSWORD=${USER_PASSWORDS[testdesigner]:-}
TESTUSER_PASSWORD=${USER_PASSWORDS[testuser]:-}
EOF

echo "    Written to dev/kanidm/.env"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=== Kanidm bootstrap complete ==="
echo ""
echo "  Kanidm URL:         $KANIDM_URL"
echo "  Admin password:     $ADMIN_PASS"
echo "  IDM Admin password: $IDM_ADMIN_PASS"
echo ""
echo "  Test users:"
echo "    testadmin    / ${USER_PASSWORDS[testadmin]:-???}  (hearth-admins, hearth-users)"
echo "    testdev      / ${USER_PASSWORDS[testdev]:-???}  (hearth-developers, hearth-users)"
echo "    testdesigner / ${USER_PASSWORDS[testdesigner]:-???}  (hearth-designers, hearth-users)"
echo "    testuser     / ${USER_PASSWORDS[testuser]:-???}  (hearth-users)"
echo ""
echo "  OAuth2 clients:"
echo "    hearth-console     (public + PKCE, web SPA)"
echo "    hearth-enrollment  (public + PKCE, enrollment kiosk)"
echo ""
echo "  Load env vars: source dev/kanidm/.env"
echo ""
