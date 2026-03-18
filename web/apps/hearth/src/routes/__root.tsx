import { Outlet, useRouter } from '@tanstack/react-router';
import { useMemo } from 'react';
import { Sidebar } from '@hearth/ui';
import type { SidebarItem } from '@hearth/ui';
import {
  LuLayoutDashboard,
  LuMonitor,
  LuUserPlus,
  LuLayers,
  LuBookOpen,
  LuFilePlus2,
  LuFileText,
  LuBarChart3,
  LuFlame,
  LuLogOut,
  LuSettings,
  LuShield,
} from 'react-icons/lu';
import { useAuth } from '../useAuth';
import { useRoles } from '../hooks/useRoles';

// All users see the catalog
const userItems: SidebarItem[] = [
  { id: 'catalog', label: 'Software Catalog', icon: <LuBookOpen size={18} /> },
];

// Admins/operators see the full admin nav
const adminItems: SidebarItem[] = [
  { id: 'dashboard', label: 'Dashboard', icon: <LuLayoutDashboard size={18} /> },
  { id: 'machines', label: 'Machines', icon: <LuMonitor size={18} /> },
  { id: 'enrollment', label: 'Enrollment', icon: <LuUserPlus size={18} /> },
  { id: 'deployments', label: 'Deployments', icon: <LuLayers size={18} /> },
  { id: 'catalog-manage', label: 'Manage Catalog', icon: <LuSettings size={18} /> },
  { id: 'requests', label: 'Requests', icon: <LuFilePlus2 size={18} /> },
  { id: 'audit', label: 'Audit Log', icon: <LuFileText size={18} /> },
  { id: 'compliance', label: 'Compliance', icon: <LuShield size={18} /> },
  { id: 'reports', label: 'Reports', icon: <LuBarChart3 size={18} /> },
];

const routeMap: Record<string, string> = {
  catalog: '/catalog',
  dashboard: '/dashboard',
  machines: '/machines',
  enrollment: '/enrollment',
  deployments: '/deployments',
  'catalog-manage': '/catalog/manage',
  requests: '/requests',
  audit: '/audit',
  compliance: '/compliance',
  reports: '/reports',
};

export function RootLayout() {
  const router = useRouter();
  const pathname = router.state.location.pathname;
  const { user, signOut, enabled } = useAuth();
  const { isOperator } = useRoles();

  const navItems = useMemo(() => {
    if (isOperator) return [...userItems, ...adminItems];
    return userItems;
  }, [isOperator]);

  const activeId = useMemo(() => {
    // Match most specific routes first
    if (pathname.startsWith('/catalog/manage')) return 'catalog-manage';
    for (const item of navItems) {
      const route = routeMap[item.id];
      if (route && pathname.startsWith(route)) return item.id;
    }
    return 'catalog';
  }, [pathname, navItems]);

  const displayName = user?.profile?.preferred_username
    ?? user?.profile?.name
    ?? user?.profile?.email
    ?? null;

  return (
    <div className="flex h-screen bg-[var(--color-surface-base)]">
      <div className="flex flex-col h-full">
        <Sidebar
          items={navItems}
          activeId={activeId}
          onNavigate={(id: string) => {
            const path = routeMap[id];
            if (path) router.navigate({ to: path });
          }}
          header={
            <div className="flex items-center gap-2">
              <div className="w-7 h-7 rounded-[var(--radius-sm)] bg-[var(--color-ember)] flex items-center justify-center text-white">
                <LuFlame size={16} />
              </div>
              <span className="font-semibold text-sm text-[var(--color-text-primary)]">Hearth</span>
            </div>
          }
        />
        {enabled && displayName && (
          <div className="px-4 py-3 border-t border-r border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-[var(--color-text-secondary)] truncate">
                {displayName}
              </span>
              <button
                type="button"
                onClick={signOut}
                className="text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer"
                title="Sign out"
              >
                <LuLogOut size={14} />
              </button>
            </div>
          </div>
        )}
      </div>
      <main className="flex-1 overflow-y-auto px-10 py-8">
        <div className="max-w-7xl">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
