import { useMemo } from 'react';
import { useFleetStats } from '../api/stats';
import { useDeployments } from '../api/deployments';
import { usePendingEnrollments } from '../api/enrollment';
import { usePendingRequests } from '../api/requests';

export interface Notification {
  id: string;
  title: string;
  body?: string;
  tone: 'info' | 'success' | 'warning' | 'danger';
  href?: { to: string };
}

export function useNotifications() {
  const stats = useFleetStats();
  const deployments = useDeployments();
  const enrollments = usePendingEnrollments();
  const requests = usePendingRequests();

  const items = useMemo<Notification[]>(() => {
    const out: Notification[] = [];

    if ((enrollments.data?.length ?? 0) > 0) {
      out.push({
        id: 'pending-enrollments',
        title: `${enrollments.data!.length} pending enrollment${enrollments.data!.length === 1 ? '' : 's'}`,
        body: 'Devices awaiting approval',
        tone: 'warning',
        href: { to: '/enrollment' },
      });
    }

    if ((requests.data?.length ?? 0) > 0) {
      out.push({
        id: 'pending-requests',
        title: `${requests.data!.length} software request${requests.data!.length === 1 ? '' : 's'}`,
        body: 'Awaiting your review',
        tone: 'info',
        href: { to: '/requests' },
      });
    }

    const failed = (deployments.data ?? []).filter((d) => d.status === 'failed');
    if (failed.length > 0) {
      out.push({
        id: 'failed-deployments',
        title: `${failed.length} failed deployment${failed.length === 1 ? '' : 's'}`,
        body: 'Investigate or rollback',
        tone: 'danger',
        href: { to: '/deployments' },
      });
    }

    if (stats.isError) {
      out.push({
        id: 'api-down',
        title: 'API unreachable',
        body: 'The control plane is not responding',
        tone: 'danger',
        href: { to: '/health' },
      });
    }

    return out;
  }, [enrollments.data, requests.data, deployments.data, stats.isError]);

  return { items, hasUnread: items.length > 0 };
}
