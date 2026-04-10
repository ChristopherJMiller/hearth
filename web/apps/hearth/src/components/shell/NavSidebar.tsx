import { useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { Sidebar } from '@hearth/ui';
import type { SidebarGroup, SidebarItem } from '@hearth/ui';
import { LuFlame, LuLogOut } from 'react-icons/lu';
import { useAuth } from '../../useAuth';
import { useRoles } from '../../hooks/useRoles';
import { usePendingEnrollments } from '../../api/enrollment';
import { usePendingRequests } from '../../api/requests';
import { useDeployments } from '../../api/deployments';
import { navGroups, findNavForPath, type NavItem } from './navConfig';
import { SystemPulse } from './SystemPulse';

export function NavSidebar() {
  const router = useRouter();
  const pathname = router.state.location.pathname;
  const { user, signOut, enabled } = useAuth();
  const { isOperator, isAdmin } = useRoles();

  // Live counts powering sidebar badges. These share the same query keys as
  // the pages they index into, so opening e.g. /enrollment hits warm cache.
  const pendingEnrollments = usePendingEnrollments();
  const pendingRequests = usePendingRequests();
  const deployments = useDeployments();

  const badges = useMemo<Record<string, number | undefined>>(() => {
    const failedDeploys = (deployments.data ?? []).filter(
      (d) => d.status === 'failed',
    ).length;
    return {
      enrollment: pendingEnrollments.data?.length,
      requests: pendingRequests.data?.length,
      deployments: failedDeploys || undefined,
    };
  }, [pendingEnrollments.data, pendingRequests.data, deployments.data]);

  const filteredGroups = useMemo<SidebarGroup[]>(() => {
    const canSee = (item: NavItem): boolean => {
      if (!item.requires) return true;
      if (item.requires === 'admin') return isAdmin;
      if (item.requires === 'operator') return isOperator;
      return true;
    };

    return navGroups
      .map((g) => ({
        id: g.id,
        label: g.label,
        items: g.items.filter(canSee).map((item): SidebarItem => {
          const { path: _path, requires: _r, ...rest } = item;
          return { ...rest, badge: badges[rest.id] };
        }),
      }))
      .filter((g) => g.items.length > 0);
  }, [isOperator, isAdmin, badges]);

  const activeId = useMemo(() => {
    const match = findNavForPath(pathname);
    return match?.id ?? 'catalog';
  }, [pathname]);

  const handleNavigate = (id: string) => {
    for (const group of navGroups) {
      const item = group.items.find((i) => i.id === id);
      if (item) {
        router.navigate({ to: item.path });
        return;
      }
    }
  };

  const displayName =
    (user?.profile?.preferred_username as string | undefined) ??
    (user?.profile?.name as string | undefined) ??
    (user?.profile?.email as string | undefined) ??
    null;

  return (
    <Sidebar
      groups={filteredGroups}
      activeId={activeId}
      onNavigate={handleNavigate}
      header={
        <div className="flex items-center gap-2.5">
          <div
            className="w-8 h-8 rounded-[var(--radius-sm)] flex items-center justify-center text-white shadow-[0_0_16px_rgba(233,69,96,0.35)]"
            style={{ background: 'linear-gradient(135deg,#e94560,#c73e54)' }}
          >
            <LuFlame size={18} />
          </div>
          <div className="flex flex-col leading-tight">
            <span className="font-semibold text-[var(--color-text-primary)] text-base">
              Hearth
            </span>
            <span className="text-[var(--color-text-tertiary)] text-2xs tracking-wide">
              FLEET CONTROL
            </span>
          </div>
        </div>
      }
      footer={
        <div className="flex flex-col">
          <SystemPulse />
          {enabled && displayName && (
            <div className="flex items-center justify-between gap-2 px-4 py-3 border-t border-[var(--color-border-subtle)]">
              <span
                className="text-[var(--color-text-secondary)] truncate text-xs"
                title={displayName}
              >
                {displayName}
              </span>
              <button
                type="button"
                onClick={signOut}
                className="text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer shrink-0"
                title="Sign out"
                aria-label="Sign out"
              >
                <LuLogOut size={15} />
              </button>
            </div>
          )}
        </div>
      }
    />
  );
}
