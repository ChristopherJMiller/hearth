import { Outlet, useRouter } from '@tanstack/react-router';
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
  LuFlame,
} from 'react-icons/lu';

const navItems: SidebarItem[] = [
  { id: 'dashboard', label: 'Dashboard', icon: <LuLayoutDashboard size={18} /> },
  { id: 'machines', label: 'Machines', icon: <LuMonitor size={18} /> },
  { id: 'enrollment', label: 'Enrollment', icon: <LuUserPlus size={18} /> },
  { id: 'deployments', label: 'Deployments', icon: <LuLayers size={18} /> },
  { id: 'catalog', label: 'Catalog', icon: <LuBookOpen size={18} /> },
  { id: 'requests', label: 'Requests', icon: <LuFilePlus2 size={18} /> },
  { id: 'audit', label: 'Audit Log', icon: <LuFileText size={18} /> },
];

const routeMap: Record<string, string> = {
  dashboard: '/dashboard',
  machines: '/machines',
  enrollment: '/enrollment',
  deployments: '/deployments',
  catalog: '/catalog',
  requests: '/requests',
  audit: '/audit',
};

export function RootLayout() {
  const router = useRouter();
  const pathname = router.state.location.pathname;

  const activeId =
    navItems.find((item) => pathname.startsWith(`/console/${item.id}`))?.id ?? 'dashboard';

  return (
    <div className="flex h-screen bg-[var(--color-surface-base)]">
      <Sidebar
        items={navItems}
        activeId={activeId}
        onNavigate={(id) => {
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
      <main className="flex-1 overflow-y-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}
