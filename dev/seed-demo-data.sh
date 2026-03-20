#!/usr/bin/env bash
# Populate database with demo data. Idempotent — safe to run multiple times.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_URL="${DATABASE_URL:-postgres://hearth:hearth@localhost:5432/hearth}"

echo "==> Seeding demo data..."
psql "$DB_URL" -f "$SCRIPT_DIR/seed-demo-data.sql" --quiet
echo "    Demo data seeded (8 machines, 18 catalog entries, 4 deployments, 25 audit events)"
