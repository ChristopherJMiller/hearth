import { StatCard, PageHeader } from '@hearth/ui';
import { useFleetStats } from '../api/stats';
import { useDeployments } from '../api/deployments';
import { usePendingEnrollments } from '../api/enrollment';
import { usePendingRequests } from '../api/requests';
import { useRouter } from '@tanstack/react-router';
import {
  LuMonitor,
  LuCheckCircle,
  LuUserPlus,
  LuLayers,
  LuInbox,
  LuChevronRight,
} from 'react-icons/lu';

const statusColors: Record<string, string> = {
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  canary: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  rolling: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  completed: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
  rolled_back: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

export function DashboardPage() {
  const router = useRouter();
  const { data: stats } = useFleetStats();
  const { data: recentDeploys } = useDeployments();
  const { data: pending } = usePendingEnrollments();
  const { data: requests } = usePendingRequests();

  const recent = recentDeploys?.slice(0, 5) ?? [];

  return (
    <div>
      <PageHeader title="Dashboard" description="Fleet overview and recent activity" />

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4 mb-8">
        <StatCard icon={<LuMonitor size={20} />} value={stats?.total_machines ?? '—'} label="Total Machines" />
        <StatCard icon={<LuCheckCircle size={20} />} value={stats?.active_machines ?? '—'} label="Active" />
        <StatCard icon={<LuUserPlus size={20} />} value={stats?.pending_enrollments ?? '—'} label="Pending Enrollment" />
        <StatCard icon={<LuLayers size={20} />} value={stats?.active_deployments ?? '—'} label="Active Deployments" />
        <StatCard icon={<LuInbox size={20} />} value={stats?.pending_requests ?? '—'} label="Pending Requests" />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Recent deployments */}
        <div className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] shadow-[var(--shadow-card)]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border-subtle)]">
            <h2 className="text-sm font-semibold text-[var(--color-text-primary)]">Recent Deployments</h2>
            <button
              type="button"
              onClick={() => router.navigate({ to: '/deployments' })}
              className="text-xs text-[var(--color-ember)] hover:underline cursor-pointer"
            >
              View all
            </button>
          </div>
          <div className="divide-y divide-[var(--color-border-subtle)]">
            {recent.length === 0 ? (
              <p className="text-sm text-[var(--color-text-tertiary)] px-5 py-8 text-center">No deployments yet</p>
            ) : (
              recent.map((d) => (
                <button
                  key={d.id}
                  type="button"
                  onClick={() => router.navigate({ to: '/deployments/$deploymentId', params: { deploymentId: d.id } })}
                  className="flex items-center justify-between w-full px-5 py-3 hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer text-left"
                >
                  <div>
                    <p className="text-sm text-[var(--color-text-primary)] font-mono truncate max-w-[280px]">
                      {d.closure.split('/').pop() ?? d.closure}
                    </p>
                    <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5">
                      {d.succeeded}/{d.total_machines} machines
                    </p>
                  </div>
                  <span className={`text-xs font-medium px-2 py-0.5 rounded-full ${statusColors[d.status] ?? ''}`}>
                    {d.status}
                  </span>
                </button>
              ))
            )}
          </div>
        </div>

        {/* Pending actions */}
        <div className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] shadow-[var(--shadow-card)]">
          <div className="px-5 py-4 border-b border-[var(--color-border-subtle)]">
            <h2 className="text-sm font-semibold text-[var(--color-text-primary)]">Pending Actions</h2>
          </div>
          <div className="divide-y divide-[var(--color-border-subtle)]">
            {(pending?.length ?? 0) > 0 && (
              <button
                type="button"
                onClick={() => router.navigate({ to: '/enrollment' })}
                className="flex items-center justify-between w-full px-5 py-3 hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-full bg-[var(--color-warning-faint)] flex items-center justify-center text-[var(--color-warning)]">
                    <LuUserPlus size={16} />
                  </div>
                  <div className="text-left">
                    <p className="text-sm text-[var(--color-text-primary)]">{pending!.length} pending enrollment{pending!.length !== 1 ? 's' : ''}</p>
                    <p className="text-xs text-[var(--color-text-tertiary)]">Awaiting approval</p>
                  </div>
                </div>
                <LuChevronRight size={16} className="text-[var(--color-text-tertiary)]" />
              </button>
            )}
            {(requests?.length ?? 0) > 0 && (
              <button
                type="button"
                onClick={() => router.navigate({ to: '/requests' })}
                className="flex items-center justify-between w-full px-5 py-3 hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-full bg-[var(--color-info-faint)] flex items-center justify-center text-[var(--color-info)]">
                    <LuInbox size={16} />
                  </div>
                  <div className="text-left">
                    <p className="text-sm text-[var(--color-text-primary)]">{requests!.length} software request{requests!.length !== 1 ? 's' : ''}</p>
                    <p className="text-xs text-[var(--color-text-tertiary)]">Awaiting review</p>
                  </div>
                </div>
                <LuChevronRight size={16} className="text-[var(--color-text-tertiary)]" />
              </button>
            )}
            {(pending?.length ?? 0) === 0 && (requests?.length ?? 0) === 0 && (
              <p className="text-sm text-[var(--color-text-tertiary)] px-5 py-8 text-center">No pending actions</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
