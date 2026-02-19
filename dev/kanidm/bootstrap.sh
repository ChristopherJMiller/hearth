#!/usr/bin/env bash
# dev/kanidm/bootstrap.sh — Provision Kanidm with Hearth identity structure
#
# Idempotent: safe to run multiple times. Creates groups, service accounts,
# and OAuth2 clients needed by the Hearth platform.
#
# Prerequisites:
#   - Kanidm container running (docker-compose up kanidm)
#   - `kanidm` CLI available (provided by nix develop shell)
#
# Usage:
#   bash dev/kanidm/bootstrap.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KANIDM_URL="${KANIDM_URL:-https://localhost:8443}"
KANIDM_CONTAINER="${KANIDM_CONTAINER:-hearth-kanidm-1}"

# Accept self-signed cert for dev
export KANIDM_SKIP_CERT_CHECK=true

echo "==> Kanidm bootstrap for Hearth"
echo "    URL: $KANIDM_URL"

# ---------------------------------------------------------------------------
# Step 1: Recover admin credentials
# ---------------------------------------------------------------------------
echo ""
echo "==> Recovering admin account..."
ADMIN_PASS=$(docker exec "$KANIDM_CONTAINER" kanidmd recover-account admin -o json 2>/dev/null \
    | grep -oP '"password"\s*:\s*"\K[^"]+' || true)

if [ -z "$ADMIN_PASS" ]; then
    echo "    Could not recover admin password. Kanidm may not be ready."
    echo "    Try: docker-compose up -d kanidm && sleep 5 && re-run this script."
    exit 1
fi

echo "    Admin password recovered"

# Authenticate as admin
kanidm login --name admin --password "$ADMIN_PASS" --url "$KANIDM_URL" 2>/dev/null || \
    kanidm login -D admin --url "$KANIDM_URL" <<< "$ADMIN_PASS" 2>/dev/null || true

# Helper: create group if it doesn't exist
create_group() {
    local name="$1"
    local desc="${2:-}"
    if kanidm group get "$name" --url "$KANIDM_URL" -D admin >/dev/null 2>&1; then
        echo "    Group '$name' already exists"
    else
        kanidm group create "$name" --url "$KANIDM_URL" -D admin 2>/dev/null
        echo "    Created group '$name'"
    fi
}

# Helper: create a person account if it doesn't exist
create_person() {
    local name="$1"
    local display="$2"
    if kanidm person get "$name" --url "$KANIDM_URL" -D admin >/dev/null 2>&1; then
        echo "    Person '$name' already exists"
    else
        kanidm person create "$name" "$display" --url "$KANIDM_URL" -D admin 2>/dev/null
        echo "    Created person '$name'"
    fi
}

# ---------------------------------------------------------------------------
# Step 2: Create groups
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating groups..."

create_group "hearth-users"       "All Hearth platform users"
create_group "hearth-admins"      "Hearth fleet administrators"
create_group "hearth-developers"  "Developer role group"
create_group "hearth-designers"   "Designer role group"

# hearth-admins should be a member of hearth-users
kanidm group add-members hearth-users --url "$KANIDM_URL" -D admin hearth-admins 2>/dev/null || true

# ---------------------------------------------------------------------------
# Step 3: Create a dev test user
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating dev test user..."

create_person "testadmin" "Test Admin"
kanidm group add-members hearth-admins --url "$KANIDM_URL" -D admin testadmin 2>/dev/null || true
kanidm group add-members hearth-users --url "$KANIDM_URL" -D admin testadmin 2>/dev/null || true

# Set a password for the test user
TEST_PASS="devpassword123"
kanidm person credential update testadmin --url "$KANIDM_URL" -D admin \
    --new-password "$TEST_PASS" 2>/dev/null || true

create_person "testdev" "Test Developer"
kanidm group add-members hearth-developers --url "$KANIDM_URL" -D admin testdev 2>/dev/null || true
kanidm group add-members hearth-users --url "$KANIDM_URL" -D admin testdev 2>/dev/null || true
kanidm person credential update testdev --url "$KANIDM_URL" -D admin \
    --new-password "$TEST_PASS" 2>/dev/null || true

# ---------------------------------------------------------------------------
# Step 4: Create service account for API → Kanidm communication
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating service account..."

if kanidm service-account get hearth-api-svc --url "$KANIDM_URL" -D admin >/dev/null 2>&1; then
    echo "    Service account 'hearth-api-svc' already exists"
else
    kanidm service-account create hearth-api-svc "Hearth API service account" \
        --url "$KANIDM_URL" -D admin 2>/dev/null
    echo "    Created service account 'hearth-api-svc'"
fi

# Generate an API token for the service account
API_TOKEN=$(kanidm service-account api-token generate hearth-api-svc "hearth-api" \
    --url "$KANIDM_URL" -D admin 2>/dev/null || echo "")

# ---------------------------------------------------------------------------
# Step 5: Register OAuth2 resource servers
# ---------------------------------------------------------------------------
echo ""
echo "==> Registering OAuth2 clients..."

# hearth-console: Authorization Code + PKCE for the web admin console
if kanidm system oauth2 get hearth-console --url "$KANIDM_URL" -D admin >/dev/null 2>&1; then
    echo "    OAuth2 client 'hearth-console' already exists"
else
    kanidm system oauth2 create hearth-console "Hearth Admin Console" \
        "https://localhost:3000/console/callback" \
        --url "$KANIDM_URL" -D admin 2>/dev/null
    echo "    Created OAuth2 client 'hearth-console'"
fi

# Allow PKCE for the console (SPA — no client secret)
kanidm system oauth2 update-scope-map hearth-console hearth-users \
    openid profile email groups \
    --url "$KANIDM_URL" -D admin 2>/dev/null || true
kanidm system oauth2 enable-pkce hearth-console \
    --url "$KANIDM_URL" -D admin 2>/dev/null || true
kanidm system oauth2 enable-localhost-redirects hearth-console \
    --url "$KANIDM_URL" -D admin 2>/dev/null || true

# hearth-enrollment: Device Authorization Grant for the enrollment TUI
if kanidm system oauth2 get hearth-enrollment --url "$KANIDM_URL" -D admin >/dev/null 2>&1; then
    echo "    OAuth2 client 'hearth-enrollment' already exists"
else
    kanidm system oauth2 create hearth-enrollment "Hearth Device Enrollment" \
        "https://localhost:8443" \
        --url "$KANIDM_URL" -D admin 2>/dev/null
    echo "    Created OAuth2 client 'hearth-enrollment'"
fi

kanidm system oauth2 update-scope-map hearth-enrollment hearth-users \
    openid profile groups \
    --url "$KANIDM_URL" -D admin 2>/dev/null || true

# ---------------------------------------------------------------------------
# Step 6: Write .env for local dev
# ---------------------------------------------------------------------------
echo ""
echo "==> Writing dev environment file..."

CONSOLE_SECRET=$(kanidm system oauth2 show-basic-secret hearth-console \
    --url "$KANIDM_URL" -D admin 2>/dev/null || echo "pkce-no-secret")

cat > "$SCRIPT_DIR/.env" <<EOF
# Generated by bootstrap.sh — do not commit
KANIDM_URL=$KANIDM_URL
KANIDM_ADMIN_PASSWORD=$ADMIN_PASS
KANIDM_OIDC_ISSUER=${KANIDM_URL}/oauth2/openid/hearth-console
KANIDM_OIDC_AUDIENCE=hearth-console
KANIDM_ENROLLMENT_CLIENT_ID=hearth-enrollment
KANIDM_CONSOLE_CLIENT_ID=hearth-console
KANIDM_CONSOLE_SECRET=$CONSOLE_SECRET
HEARTH_API_SVC_TOKEN=$API_TOKEN
TEST_USER_PASSWORD=$TEST_PASS
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
echo ""
echo "  Test users:"
echo "    testadmin / $TEST_PASS  (hearth-admins, hearth-users)"
echo "    testdev   / $TEST_PASS  (hearth-developers, hearth-users)"
echo ""
echo "  OAuth2 clients:"
echo "    hearth-console     (PKCE, web admin console)"
echo "    hearth-enrollment  (device flow, enrollment TUI)"
echo ""
echo "  Load env vars: source dev/kanidm/.env"
echo ""
