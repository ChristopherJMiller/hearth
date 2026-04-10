#!/usr/bin/env bash
# dev/caddy/bootstrap.sh — Copy Caddy's internal root CA out of the container
# so enrolled NixOS VMs can trust the *.hearth.local certs it issues.
#
# Run after `docker compose up -d caddy`. Idempotent.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CA_SRC="$SCRIPT_DIR/data/caddy/pki/authorities/local/root.crt"
CA_DST="$SCRIPT_DIR/root.crt"

# Trigger a request so Caddy issues certs and writes its root to disk.
# The call may fail (upstream not up yet) — we only care that Caddy responded.
curl -sk -o /dev/null --max-time 2 https://api.hearth.local/ --resolve api.hearth.local:443:127.0.0.1 || true

# Wait up to 30s for the root cert to appear on disk.
for _ in $(seq 1 30); do
    if [ -f "$CA_SRC" ]; then
        break
    fi
    sleep 1
done

if [ ! -f "$CA_SRC" ]; then
    echo "ERROR: Caddy did not produce a root CA at $CA_SRC" >&2
    echo "       Is the caddy container running? Check: docker compose logs caddy" >&2
    exit 1
fi

cp "$CA_SRC" "$CA_DST"
chmod 644 "$CA_DST"
echo "    Dev CA: $CA_DST"
