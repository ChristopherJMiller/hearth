#!/usr/bin/env bash
# Design system guardrails. Run from the web/ workspace root.
#
# Fails on common drift patterns:
#   - hardcoded text-[Npx] arbitrary sizes outside of tests
#   - 'console-admin' literal (use useActor() instead)
#   - p-6 outer wrappers in route files (PageContainer + main own page padding)
#   - hardcoded #hex colors in component files (use design tokens)

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_SRC="$ROOT/apps/hearth/src"
UI_SRC="$ROOT/packages/ui/src"
fail=0

check() {
  local label="$1"
  shift
  local matches
  if matches=$(grep -RnE "$@" 2>/dev/null); then
    if [[ -n "$matches" ]]; then
      echo
      echo "❌ $label"
      echo "$matches" | head -50
      fail=1
    fi
  fi
}

# 1. Arbitrary text sizes — should use --text-* tokens via Tailwind defaults.
check "ad-hoc text-[Npx] sizes (use --text-* tokens)" \
  "text-\\[[0-9]+px\\]" "$APP_SRC" "$UI_SRC"

# 2. The hardcoded actor literal.
check "'console-admin' literal (use useActor())" \
  "console-admin" "$APP_SRC"

# 3. p-6 outer wrappers on route files (PageContainer/main own padding now).
if matches=$(grep -RnE "^\s*<div className=\"p-6" "$APP_SRC/routes" 2>/dev/null); then
  if [[ -n "$matches" ]]; then
    echo
    echo "❌ p-6 outer wrapper in route file (PageContainer owns padding)"
    echo "$matches"
    fail=1
  fi
fi

# 4. Hardcoded hex colors in @hearth/ui components — tokens only.
check "hardcoded hex color in @hearth/ui component (use --color-* tokens)" \
  "#[0-9a-fA-F]{6}" "$UI_SRC/components"

if [[ $fail -ne 0 ]]; then
  echo
  echo "Design system guardrails failed. Fix the issues above before merging."
  exit 1
fi

echo "✅ Design system guardrails clean."
