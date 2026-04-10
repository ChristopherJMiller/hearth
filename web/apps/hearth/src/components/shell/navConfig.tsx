import type { ReactNode } from 'react';
import type { SidebarGroup, SidebarItem } from '@hearth/ui';
import {
  LuLayoutDashboard,
  LuMonitor,
  LuUserPlus,
  LuNetwork,
  LuLayers,
  LuHammer,
  LuBookOpen,
  LuFilePlus2,
  LuSettings2,
  LuUsers,
  LuContact,
  LuFileText,
  LuActivity,
  LuShield,
  LuBarChart3,
  LuGlobe,
  LuUser,
  LuSettings,
} from 'react-icons/lu';

export interface NavItem extends SidebarItem {
  path: string;
  requires?: 'operator' | 'admin';
}

export interface NavGroup extends Omit<SidebarGroup, 'items'> {
  items: NavItem[];
}

const icon = (Icon: React.ComponentType<{ size?: number }>): ReactNode => <Icon size={18} />;

/**
 * Canonical nav config. The single source of truth for sidebar groups,
 * breadcrumb labels, and the command palette's navigation entries.
 * Per-item `requires` gates visibility by role.
 */
export const navGroups: NavGroup[] = [
  {
    id: 'fleet',
    label: 'Fleet',
    items: [
      { id: 'dashboard', label: 'Overview', icon: icon(LuLayoutDashboard), path: '/dashboard', requires: 'operator' },
      { id: 'machines', label: 'Machines', icon: icon(LuMonitor), path: '/machines', requires: 'operator' },
      { id: 'enrollment', label: 'Enrollment', icon: icon(LuUserPlus), path: '/enrollment', requires: 'operator' },
      { id: 'mesh', label: 'Mesh', icon: icon(LuNetwork), path: '/mesh', requires: 'operator' },
    ],
  },
  {
    id: 'software',
    label: 'Software',
    items: [
      { id: 'deployments', label: 'Deployments', icon: icon(LuLayers), path: '/deployments', requires: 'operator' },
      { id: 'builds', label: 'Build Queue', icon: icon(LuHammer), path: '/builds', requires: 'operator' },
      { id: 'catalog', label: 'Catalog', icon: icon(LuBookOpen), path: '/catalog' },
      { id: 'catalog-manage', label: 'Manage Catalog', icon: icon(LuSettings2), path: '/catalog/manage', requires: 'operator' },
      { id: 'requests', label: 'Requests', icon: icon(LuFilePlus2), path: '/requests', requires: 'operator' },
    ],
  },
  {
    id: 'identity',
    label: 'Identity & Access',
    items: [
      { id: 'people', label: 'People', icon: icon(LuContact), path: '/people', requires: 'operator' },
      { id: 'directory', label: 'Directory', icon: icon(LuUsers), path: '/directory' },
      { id: 'audit', label: 'Audit Log', icon: icon(LuFileText), path: '/audit', requires: 'operator' },
    ],
  },
  {
    id: 'observability',
    label: 'Observability',
    items: [
      { id: 'health', label: 'Health', icon: icon(LuActivity), path: '/health', requires: 'operator' },
      { id: 'compliance', label: 'Compliance', icon: icon(LuShield), path: '/compliance', requires: 'operator' },
      { id: 'reports', label: 'Reports', icon: icon(LuBarChart3), path: '/reports', requires: 'operator' },
      { id: 'services', label: 'Services', icon: icon(LuGlobe), path: '/services' },
    ],
  },
  {
    id: 'personal',
    label: 'Personal',
    items: [
      { id: 'me-environment', label: 'My Environment', icon: icon(LuUser), path: '/me/environment' },
      { id: 'settings', label: 'Settings', icon: icon(LuSettings), path: '/settings' },
    ],
  },
];

export const flatNav: NavItem[] = navGroups.flatMap((g) => g.items);

export function findNavForPath(pathname: string): NavItem | undefined {
  // Most specific (longest) match wins.
  const sorted = [...flatNav].sort((a, b) => b.path.length - a.path.length);
  return sorted.find((item) => pathname === item.path || pathname.startsWith(`${item.path}/`));
}

export function buildBreadcrumbs(pathname: string): { label: string; path?: string }[] {
  const match = findNavForPath(pathname);
  if (!match) return [{ label: 'Home' }];

  // Find the group label for context.
  const group = navGroups.find((g) => g.items.some((i) => i.id === match.id));
  const crumbs: { label: string; path?: string }[] = [];
  if (group) crumbs.push({ label: group.label });
  crumbs.push({ label: match.label, path: match.path });

  // Detail sub-segments: show a trailing crumb from the remaining path.
  const remainder = pathname.slice(match.path.length).replace(/^\//, '');
  if (remainder) {
    const tail = remainder.split('/').filter(Boolean);
    for (const seg of tail) {
      if (seg === 'new') {
        crumbs.push({ label: 'New' });
      } else if (seg === 'manage') {
        // already covered by a dedicated nav item
      } else {
        crumbs.push({ label: seg });
      }
    }
  }
  return crumbs;
}
