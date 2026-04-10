import { useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { CommandPalette, type CommandItem } from '@hearth/ui';
import {
  LuLayoutDashboard,
  LuMonitor,
  LuRocket,
  LuUserPlus,
  LuHammer,
  LuBookOpen,
  LuFileText,
  LuActivity,
  LuShield,
  LuBarChart3,
  LuGlobe,
  LuUser,
  LuSettings,
  LuLayers,
  LuFilePlus2,
  LuContact,
  LuUsers,
  LuNetwork,
  LuSettings2,
  LuLock,
} from 'react-icons/lu';
import { useShell } from './ShellContext';
import { useMachines } from '../../api/machines';
import { useDeployments, useRollbackDeployment } from '../../api/deployments';
import { createMachineAction } from '../../api/actions';
import { truncateId } from '../../lib/time';

interface NavCommand {
  id: string;
  label: string;
  group: string;
  icon: React.ReactNode;
  to: string;
  hint?: string;
}

const navCommands: NavCommand[] = [
  { id: 'nav-dashboard', label: 'Dashboard', group: 'Navigation', icon: <LuLayoutDashboard size={15} />, to: '/dashboard' },
  { id: 'nav-machines', label: 'Machines', group: 'Navigation', icon: <LuMonitor size={15} />, to: '/machines' },
  { id: 'nav-enrollment', label: 'Enrollment', group: 'Navigation', icon: <LuUserPlus size={15} />, to: '/enrollment' },
  { id: 'nav-mesh', label: 'Mesh', group: 'Navigation', icon: <LuNetwork size={15} />, to: '/mesh' },
  { id: 'nav-deployments', label: 'Deployments', group: 'Navigation', icon: <LuLayers size={15} />, to: '/deployments' },
  { id: 'nav-builds', label: 'Build queue', group: 'Navigation', icon: <LuHammer size={15} />, to: '/builds' },
  { id: 'nav-catalog', label: 'Catalog', group: 'Navigation', icon: <LuBookOpen size={15} />, to: '/catalog' },
  { id: 'nav-catalog-manage', label: 'Manage catalog', group: 'Navigation', icon: <LuSettings2 size={15} />, to: '/catalog/manage' },
  { id: 'nav-requests', label: 'Software requests', group: 'Navigation', icon: <LuFilePlus2 size={15} />, to: '/requests' },
  { id: 'nav-people', label: 'People (admin)', group: 'Navigation', icon: <LuContact size={15} />, to: '/people' },
  { id: 'nav-directory', label: 'Directory', group: 'Navigation', icon: <LuUsers size={15} />, to: '/directory' },
  { id: 'nav-audit', label: 'Audit log', group: 'Navigation', icon: <LuFileText size={15} />, to: '/audit' },
  { id: 'nav-health', label: 'Health', group: 'Navigation', icon: <LuActivity size={15} />, to: '/health' },
  { id: 'nav-compliance', label: 'Compliance', group: 'Navigation', icon: <LuShield size={15} />, to: '/compliance' },
  { id: 'nav-reports', label: 'Reports', group: 'Navigation', icon: <LuBarChart3 size={15} />, to: '/reports' },
  { id: 'nav-services', label: 'Services', group: 'Navigation', icon: <LuGlobe size={15} />, to: '/services' },
  { id: 'nav-me', label: 'My environment', group: 'Navigation', icon: <LuUser size={15} />, to: '/me/environment' },
  { id: 'nav-settings', label: 'Settings', group: 'Navigation', icon: <LuSettings size={15} />, to: '/settings' },
];

export function CommandPaletteHost() {
  const router = useRouter();
  const { commandPaletteOpen, closeCommandPalette } = useShell();
  const { data: machines } = useMachines();
  const { data: deployments } = useDeployments();
  const rollback = useRollbackDeployment();

  const items = useMemo<CommandItem[]>(() => {
    const navItems: CommandItem[] = navCommands.map((cmd) => ({
      id: cmd.id,
      label: cmd.label,
      group: cmd.group,
      icon: cmd.icon,
      hint: cmd.hint,
      keywords: cmd.label,
      onRun: () => router.navigate({ to: cmd.to }),
    }));

    const actionItems: CommandItem[] = [
      {
        id: 'action-new-deployment',
        label: 'Create deployment',
        group: 'Actions',
        icon: <LuRocket size={15} />,
        keywords: 'new ship rollout',
        onRun: () => router.navigate({ to: '/deployments/new' }),
      },
    ];

    const latestActiveDeployment = deployments?.find(
      (d) => d.status === 'canary' || d.status === 'rolling',
    );
    if (latestActiveDeployment) {
      actionItems.push({
        id: 'action-rollback-latest',
        label: `Rollback latest deployment (${truncateId(latestActiveDeployment.id)})`,
        group: 'Actions',
        icon: <LuRocket size={15} />,
        keywords: 'undo revert halt',
        onRun: () => rollback.mutate(latestActiveDeployment.id),
      });
    }

    const machineItems: CommandItem[] = (machines ?? []).slice(0, 50).map((m) => ({
      id: `machine-${m.id}`,
      label: `Open ${m.hostname}`,
      group: 'Machines',
      icon: <LuMonitor size={15} />,
      hint: m.role ?? undefined,
      keywords: `${m.hostname} ${m.role ?? ''} ${m.tags.join(' ')}`,
      onRun: () =>
        router.navigate({
          to: '/machines/$machineId',
          params: { machineId: m.id },
        }),
    }));

    const lockItems: CommandItem[] = (machines ?? []).slice(0, 25).map((m) => ({
      id: `lock-${m.id}`,
      label: `Lock ${m.hostname}`,
      group: 'Machine actions',
      icon: <LuLock size={15} />,
      keywords: `lock secure ${m.hostname}`,
      onRun: () => {
        createMachineAction(m.id, { action_type: 'lock' }).catch((err) => {
          console.error('Failed to lock machine', m.id, err);
        });
      },
    }));

    return [...actionItems, ...navItems, ...machineItems, ...lockItems];
  }, [router, machines, deployments, rollback]);

  return (
    <CommandPalette
      open={commandPaletteOpen}
      onOpenChange={(open) => !open && closeCommandPalette()}
      items={items}
      placeholder="Search machines, deployments, or jump to a page…"
    />
  );
}
