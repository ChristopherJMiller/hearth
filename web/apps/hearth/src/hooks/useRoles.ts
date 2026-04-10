import { useAuth } from '../useAuth';

export type HearthRole = 'viewer' | 'operator' | 'admin';

/**
 * Normalize Kanidm's OIDC `groups` claim. Kanidm emits each group twice —
 * once as a UUID and once as an SPN (`name@domain`). Drop the UUID form and
 * strip the `@domain` suffix so role checks can match on plain short names.
 * Mirrors `normalize_groups` in `crates/hearth-api/src/auth.rs`.
 */
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function normalizeGroups(raw: unknown): string[] {
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const g of raw) {
    if (typeof g !== 'string' || UUID_RE.test(g)) continue;
    const at = g.indexOf('@');
    const short = at === -1 ? g : g.slice(0, at);
    if (!seen.has(short)) {
      seen.add(short);
      out.push(short);
    }
  }
  return out;
}

/**
 * Derives role flags from the authenticated user's Kanidm groups.
 *
 * When auth is disabled entirely (dev mode without an OIDC authority),
 * everyone is treated as an admin so the full nav and all admin workflows
 * are reachable locally. In production, role gating is driven by actual
 * Kanidm group membership: `hearth-admins`, `hearth-operators`,
 * `hearth-viewers`.
 */
export function useRoles() {
  const { user, enabled } = useAuth();

  if (!enabled) {
    return {
      role: 'admin' as HearthRole,
      isAdmin: true,
      isOperator: true,
      isViewer: true,
      groups: [] as string[],
    };
  }

  const groups = normalizeGroups(user?.profile?.groups);

  const isAdmin = groups.includes('hearth-admins');
  const isOperator = isAdmin || groups.includes('hearth-operators');
  const isViewer = isOperator || groups.includes('hearth-viewers');

  const role: HearthRole = isAdmin ? 'admin' : isOperator ? 'operator' : 'viewer';

  return { role, isAdmin, isOperator, isViewer, groups };
}
