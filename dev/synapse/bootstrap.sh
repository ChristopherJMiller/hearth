#!/usr/bin/env bash
# dev/synapse/bootstrap.sh — Create admin bot and default rooms in Synapse
#
# Idempotent: safe to run multiple times. Creates an admin bot user and
# default corporate chat rooms.
#
# Prerequisites:
#   - Synapse running and healthy (docker-compose up synapse)
#   - Kanidm bootstrap already run (OIDC client secret written to .env)
#
# Usage:
#   bash dev/synapse/bootstrap.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYNAPSE_URL="${SYNAPSE_URL:-http://localhost:8008}"
SHARED_SECRET="hearth-dev-synapse-secret-change-in-prod"
SERVER_NAME="hearth.local"

echo "==> Synapse bootstrap for Hearth Chat"
echo "    URL: $SYNAPSE_URL"

# ---------------------------------------------------------------------------
# Write OIDC client secret file into the Synapse container
# ---------------------------------------------------------------------------
if [ -f "$SCRIPT_DIR/../kanidm/.env" ]; then
    MATRIX_SECRET=$(grep '^MATRIX_OIDC_CLIENT_SECRET=' "$SCRIPT_DIR/../kanidm/.env" 2>/dev/null | cut -d= -f2 || true)
    if [ -n "$MATRIX_SECRET" ]; then
        SYNAPSE_CONTAINER="${SYNAPSE_CONTAINER:-hearth-synapse-1}"
        printf '%s' "$MATRIX_SECRET" | docker exec -i "$SYNAPSE_CONTAINER" sh -c 'cat > /data/oidc_client_secret' 2>/dev/null || true
        echo "    Wrote OIDC client secret to Synapse container"
    fi
fi

# ---------------------------------------------------------------------------
# Helper: register user via shared secret (Synapse admin API)
# ---------------------------------------------------------------------------
register_user() {
    local username="$1"
    local password="$2"
    local admin="${3:-false}"

    # Generate the HMAC nonce
    local nonce
    nonce=$(curl -s "$SYNAPSE_URL/_synapse/admin/v1/register" | jq -r '.nonce')

    if [ -z "$nonce" ] || [ "$nonce" = "null" ]; then
        echo "    ERROR: Could not get registration nonce"
        return 1
    fi

    # Compute HMAC-SHA1: nonce\0username\0password\0admin|notadmin
    local admin_str="notadmin"
    [ "$admin" = "true" ] && admin_str="admin"
    local mac
    mac=$(printf '%s\0%s\0%s\0%s' "$nonce" "$username" "$password" "$admin_str" \
        | openssl dgst -sha1 -hmac "$SHARED_SECRET" | awk '{print $NF}')

    local response
    response=$(curl -s -X POST "$SYNAPSE_URL/_synapse/admin/v1/register" \
        -H "Content-Type: application/json" \
        -d "$(jq -n \
            --arg nonce "$nonce" \
            --arg user "$username" \
            --arg pass "$password" \
            --arg mac "$mac" \
            --argjson admin "$admin" \
            '{nonce: $nonce, username: $user, password: $pass, mac: $mac, admin: $admin}')")

    if echo "$response" | jq -e '.access_token' > /dev/null 2>&1; then
        echo "    Created user '$username'"
        echo "$response" | jq -r '.access_token'
    elif echo "$response" | grep -q "User ID already taken"; then
        echo "    User '$username' already exists"
        # Login to get access token
        local login_resp
        login_resp=$(curl -s -X POST "$SYNAPSE_URL/_matrix/client/v3/login" \
            -H "Content-Type: application/json" \
            -d "$(jq -n \
                --arg user "$username" \
                --arg pass "$password" \
                '{type: "m.login.password", identifier: {type: "m.id.user", user: $user}, password: $pass}')")
        echo "$login_resp" | jq -r '.access_token // empty'
    else
        echo "    ERROR registering '$username': $response"
        return 1
    fi
}

# ---------------------------------------------------------------------------
# Helper: create room (idempotent via alias check)
# ---------------------------------------------------------------------------
create_room() {
    local token="$1"
    local alias="$2"
    local name="$3"
    local topic="$4"

    # Check if room alias already exists
    local check
    check=$(curl -s "$SYNAPSE_URL/_matrix/client/v3/directory/room/%23${alias}:${SERVER_NAME}" \
        -H "Authorization: Bearer $token")

    if echo "$check" | jq -e '.room_id' > /dev/null 2>&1; then
        echo "    Room #${alias}:${SERVER_NAME} already exists"
        return 0
    fi

    local response
    response=$(curl -s -X POST "$SYNAPSE_URL/_matrix/client/v3/createRoom" \
        -H "Authorization: Bearer $token" \
        -H "Content-Type: application/json" \
        -d "$(jq -n \
            --arg name "$name" \
            --arg topic "$topic" \
            --arg alias "$alias" \
            '{
                name: $name,
                topic: $topic,
                room_alias_name: $alias,
                visibility: "public",
                preset: "public_chat",
                creation_content: {
                    "m.federate": false
                },
                initial_state: [
                    {
                        type: "m.room.history_visibility",
                        content: { history_visibility: "shared" }
                    },
                    {
                        type: "m.room.guest_access",
                        content: { guest_access: "forbidden" }
                    }
                ]
            }')")

    if echo "$response" | jq -e '.room_id' > /dev/null 2>&1; then
        echo "    Created room #${alias}:${SERVER_NAME}"
    else
        echo "    ERROR creating room #${alias}: $response"
    fi
}

# ---------------------------------------------------------------------------
# Helper: send message to room
# ---------------------------------------------------------------------------
send_message() {
    local token="$1"
    local room_alias="$2"
    local message="$3"

    # Resolve alias to room ID
    local room_id
    room_id=$(curl -s "$SYNAPSE_URL/_matrix/client/v3/directory/room/%23${room_alias}:${SERVER_NAME}" \
        -H "Authorization: Bearer $token" | jq -r '.room_id // empty')

    if [ -z "$room_id" ]; then
        echo "    Could not resolve #${room_alias} — skipping message"
        return 0
    fi

    # URL-encode the room ID
    local encoded_room
    encoded_room=$(printf '%s' "$room_id" | jq -sRr @uri)

    local txn_id
    txn_id=$(date +%s%N)

    curl -s -X PUT \
        "$SYNAPSE_URL/_matrix/client/v3/rooms/${encoded_room}/send/m.room.message/${txn_id}" \
        -H "Authorization: Bearer $token" \
        -H "Content-Type: application/json" \
        -d "$(jq -n --arg body "$message" '{msgtype: "m.text", body: $body}')" > /dev/null

    echo "    Sent welcome message to #${room_alias}"
}

# ---------------------------------------------------------------------------
# Step 1: Create admin bot
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating admin bot..."

BOT_TOKEN=$(register_user "hearth-bot" "hearth-bot-dev-password" true)

if [ -z "$BOT_TOKEN" ]; then
    echo "    WARNING: Could not get bot access token. Rooms may need manual creation."
    exit 0
fi

# ---------------------------------------------------------------------------
# Step 2: Create default rooms
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating default rooms..."

create_room "$BOT_TOKEN" "general" "General" "Company-wide announcements and discussion"
create_room "$BOT_TOKEN" "random" "Random" "Off-topic chat and social"
create_room "$BOT_TOKEN" "it-support" "IT Support" "IT help desk — ask questions, report issues"

# ---------------------------------------------------------------------------
# Step 3: Post welcome messages
# ---------------------------------------------------------------------------
echo ""
echo "==> Posting welcome messages..."

send_message "$BOT_TOKEN" "general" "Welcome to Hearth Chat! This is the company-wide channel for announcements and discussion. All team members are automatically added here."
send_message "$BOT_TOKEN" "it-support" "Welcome to IT Support. Post your questions here and the IT team will help you out."

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=== Synapse bootstrap complete ==="
echo ""
echo "  Synapse URL:   $SYNAPSE_URL"
echo "  Element Web:   http://localhost:8088"
echo "  Admin bot:     @hearth-bot:${SERVER_NAME}"
echo ""
echo "  Default rooms:"
echo "    #general:${SERVER_NAME}     — Company-wide"
echo "    #random:${SERVER_NAME}      — Social/off-topic"
echo "    #it-support:${SERVER_NAME}  — IT help desk"
echo ""
