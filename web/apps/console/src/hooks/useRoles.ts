import { useAuth } from '../useAuth';

export type HearthRole = 'viewer' | 'operator' | 'admin';

export function useRoles() {
  const { user } = useAuth();

  const groups: string[] = (user?.profile?.groups as string[]) ?? [];

  const isAdmin = groups.includes('hearth-admins');
  const isOperator = isAdmin || groups.includes('hearth-operators');
  const isViewer = isOperator || groups.includes('hearth-viewers');

  const role: HearthRole = isAdmin ? 'admin' : isOperator ? 'operator' : 'viewer';

  return { role, isAdmin, isOperator, isViewer, groups };
}
