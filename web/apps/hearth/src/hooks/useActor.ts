import { useAuth } from '../useAuth';

/**
 * Returns the authenticated user's actor identifier for audit-log attribution.
 * Falls back to display name, then email, then 'system' if auth is disabled.
 *
 * Use this anywhere the API expects an `approved_by`/`resolved_by`/`actor`
 * string — never hardcode the actor name.
 */
export function useActor(): string {
  const { user } = useAuth();
  const profile = user?.profile;
  return (
    (profile?.preferred_username as string | undefined) ??
    (profile?.name as string | undefined) ??
    (profile?.email as string | undefined) ??
    'system'
  );
}
