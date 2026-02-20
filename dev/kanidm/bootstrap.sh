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
        -subj "/CN=localhost" \
        -addext "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:10.0.2.2,IP:::1" \
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
$C -X POST "$KANIDM_URL/v1/group/idm_all_persons/_attr/class" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '["account_policy"]' > /dev/null 2>&1 || true

# Set credential_type_minimum to "any" (allows password-only)
$C -X PUT "$KANIDM_URL/v1/group/idm_all_persons/_attr/credential_type_minimum" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '["any"]' > /dev/null 2>&1 || true

echo "    Set credential_type_minimum=any on idm_all_persons"

# ---------------------------------------------------------------------------
# REST API helpers
# ---------------------------------------------------------------------------
# admin token: system config, OAuth2
# idm_admin token: groups, persons, credentials

idm_get() {
    $C "$KANIDM_URL$1" -H "Authorization: Bearer $IDM_TOKEN" -w '\n%{http_code}'
}

idm_post() {
    $C -X POST "$KANIDM_URL$1" \
        -H "Authorization: Bearer $IDM_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2" -w '\n%{http_code}'
}

admin_get() {
    $C "$KANIDM_URL$1" -H "Authorization: Bearer $ADMIN_TOKEN" -w '\n%{http_code}'
}

admin_post() {
    $C -X POST "$KANIDM_URL$1" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2" -w '\n%{http_code}'
}

admin_patch() {
    $C -X PATCH "$KANIDM_URL$1" \
        -H "Authorization: Bearer $ADMIN_TOKEN" \
        -H "Content-Type: application/json" \
        -d "$2" -w '\n%{http_code}'
}

# Check if resource exists (returns 0 if exists)
# Kanidm returns 200 with "null" body for missing resources, so check body too
resource_exists() {
    local body
    body=$($C "$KANIDM_URL$1" -H "Authorization: Bearer ${2:-$IDM_TOKEN}")
    [ "$body" != "null" ] && [ -n "$body" ]
}

create_group() {
    local name="$1"
    if resource_exists "/v1/group/$name"; then
        echo "    Group '$name' already exists"
    else
        idm_post "/v1/group" "{\"attrs\":{\"name\":[\"$name\"]}}" > /dev/null
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
        idm_post "/v1/person" "{\"attrs\":{\"name\":[\"$name\"],\"displayname\":[\"$display\"]}}" > /dev/null
        echo "    Created person '$name'"
    fi
}

set_person_password() {
    local name="$1"
    # Use kanidmd recover-account inside the container to set a password.
    # This bypasses credential policy checks (MFA requirements etc.)
    # and generates a random password.
    local recover_out password
    recover_out=$(docker exec "$KANIDM_CONTAINER" kanidmd recover-account "$name" 2>&1)
    password=$(echo "$recover_out" | grep -oP 'new_password:\s*"\K[^"]+' || true)
    if [ -n "$password" ]; then
        # Store the password in an associative array for the .env file
        USER_PASSWORDS["$name"]="$password"
    else
        echo "    Warning: could not set password for '$name'"
    fi
}

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
set_person_password "testadmin"

create_person "testdev" "Test Developer"
add_group_member "hearth-developers" "testdev"
add_group_member "hearth-users" "testdev"
set_person_password "testdev"

create_person "testdesigner" "Test Designer"
add_group_member "hearth-designers" "testdesigner"
add_group_member "hearth-users" "testdesigner"
set_person_password "testdesigner"

create_person "testuser" "Test User"
add_group_member "hearth-users" "testuser"
set_person_password "testuser"

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

# hearth-console: Authorization Code + PKCE for the web admin console
if resource_exists "/v1/system/oauth2/hearth-console" "$ADMIN_TOKEN"; then
    echo "    OAuth2 client 'hearth-console' already exists"
else
    admin_post "/v1/system/oauth2" \
        '{"attrs":{"oauth2_rs_name":["hearth-console"],"displayname":["Hearth Admin Console"],"oauth2_rs_origin":["https://localhost:3000"]}}' > /dev/null
    echo "    Created OAuth2 client 'hearth-console'"
fi

# Configure hearth-console
admin_post "/v1/system/oauth2/hearth-console/_scopemap/hearth-users" \
    '["openid","profile","email","groups"]' > /dev/null 2>&1 || true

# Enable PKCE + localhost redirects
admin_patch "/v1/system/oauth2/hearth-console" \
    '{"attrs":{"oauth2_allow_localhost_redirect":["true"],"oauth2_prefer_short_username":["true"]}}' > /dev/null 2>&1 || true

# hearth-enrollment: Device Authorization Grant for the enrollment TUI
if resource_exists "/v1/system/oauth2/hearth-enrollment" "$ADMIN_TOKEN"; then
    echo "    OAuth2 client 'hearth-enrollment' already exists"
else
    admin_post "/v1/system/oauth2" \
        '{"attrs":{"oauth2_rs_name":["hearth-enrollment"],"displayname":["Hearth Device Enrollment"],"oauth2_rs_origin":["https://localhost:8443"]}}' > /dev/null
    echo "    Created OAuth2 client 'hearth-enrollment'"
fi

admin_post "/v1/system/oauth2/hearth-enrollment/_scopemap/hearth-users" \
    '["openid","profile","groups"]' > /dev/null 2>&1 || true

# ---------------------------------------------------------------------------
# Step 6: Write .env for local dev
# ---------------------------------------------------------------------------
echo ""
echo "==> Writing dev environment file..."

CONSOLE_SECRET=$($C "$KANIDM_URL/v1/system/oauth2/hearth-console" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    | jq -r '.attrs.oauth2_rs_basic_secret[0] // "pkce-no-secret"' 2>/dev/null || echo "pkce-no-secret")

cat > "$SCRIPT_DIR/.env" <<EOF
# Generated by bootstrap.sh — do not commit
KANIDM_URL=$KANIDM_URL
KANIDM_ADMIN_PASSWORD=$ADMIN_PASS
KANIDM_IDM_ADMIN_PASSWORD=$IDM_ADMIN_PASS
KANIDM_OIDC_ISSUER=${KANIDM_URL}/oauth2/openid/hearth-console
KANIDM_OIDC_AUDIENCE=hearth-console
KANIDM_ENROLLMENT_CLIENT_ID=hearth-enrollment
KANIDM_CONSOLE_CLIENT_ID=hearth-console
KANIDM_CONSOLE_SECRET=$CONSOLE_SECRET
HEARTH_API_SVC_TOKEN=$API_TOKEN
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
echo "    hearth-console     (PKCE, web admin console)"
echo "    hearth-enrollment  (device flow, enrollment TUI)"
echo ""
echo "  Load env vars: source dev/kanidm/.env"
echo ""
