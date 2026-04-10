import { useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  MetricTile,
  Timeline,
  Card,
  StatusChip,
  Callout,
  SkeletonCard,
  Button,
} from '@hearth/ui';
import type { TimelineEvent } from '@hearth/ui';
import {
  LuMonitor,
  LuActivity,
  LuUserPlus,
  LuLayers,
  LuInbox,
  LuChevronRight,
  LuRocket,
  LuShieldAlert,
} from 'react-icons/lu';
import { useFleetStats } from '../api/stats';
import { useDeployments } from '../api/deployments';
import { usePendingEnrollments } from '../api/enrollment';
import { usePendingRequests } from '../api/requests';
import { useAuditLog } from '../api/audit';
import { formatRelativeTime } from '../lib/time';
import type { Deployment } from '../api/types';

type DashboardTone = 'success' | 'danger' | 'warning' | 'info' | 'default';

function deploymentTone(status: Deployment['status']): DashboardTone {
  switch (status) {
    case 'completed':
      return 'success';
    case 'failed':
      return 'danger';
    case 'rolled_back':
      return 'warning';
    case 'canary':
    case 'rolling':
      return 'info';
    default:
      return 'default';
  }
}

export function DashboardPage() {
  const router = useRouter();
  const stats = useFleetStats();
  const deployments = useDeployments();
  const pending = usePendingEnrollments();
  const requests = usePendingRequests();
  const audit = useAuditLog({ limit: 25 });

  const recent = (deployments.data ?? []).slice(0, 5);

  const timelineEvents = useMemo<TimelineEvent[]>(() => {
    // We carry the raw ISO timestamp as `sortKey` so we can sort chronologically
    // and still show the user-friendly relative time. ISO-8601 strings sort
    // lexically == chronologically.
    type Buildable = TimelineEvent & { sortKey: string };
    const events: Buildable[] = [];
    for (const d of (deployments.data ?? []).slice(0, 6)) {
      const tone = deploymentTone(d.status);
      events.push({
        id: `deploy-${d.id}`,
        sortKey: d.updated_at,
        time: formatRelativeTime(d.updated_at),
        title: (
          <span>
            Deployment <span className="font-mono text-[var(--color-text-secondary)]">{d.closure.split('/').pop()?.slice(0, 24) ?? d.closure.slice(0, 24)}</span>
          </span>
        ),
        body: (
          <span>
            {d.succeeded}/{d.total_machines} succeeded
            {d.failed > 0 && <span className="text-[var(--color-error)]"> · {d.failed} failed</span>}
          </span>
        ),
        tone: tone === 'default' ? 'info' : tone,
        icon: <LuRocket size={14} />,
        onClick: () => router.navigate({ to: '/deployments/$deploymentId', params: { deploymentId: d.id } }),
      });
    }
    for (const e of (audit.data ?? []).slice(0, 4)) {
      events.push({
        id: `audit-${e.id}`,
        sortKey: e.created_at,
        time: formatRelativeTime(e.created_at),
        title: e.event_type.replace(/[_.]/g, ' '),
        body: e.actor ? `by ${e.actor}` : undefined,
        tone: 'default',
      });
    }
    events.sort((a, b) => b.sortKey.localeCompare(a.sortKey));
    return events.slice(0, 10).map(({ sortKey: _sk, ...rest }) => rest);
  }, [deployments.data, audit.data, router]);

  const totalMachines = stats.data?.total_machines;
  const activeMachines = stats.data?.active_machines;
  const activityRate = totalMachines ? Math.round(((activeMachines ?? 0) / totalMachines) * 100) : 0;
  const operationsInboxCount =
    (pending.data?.length ?? 0) + (requests.data?.length ?? 0);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Fleet control"
        title="Overview"
        description="Live pulse on your fleet — enrollment pipeline, deployments in flight, and everything that needs your attention."
        actions={
          <>
            <Button
              variant="subtle"
              leadingIcon={<LuInbox size={15} />}
              onClick={() => router.navigate({ to: '/audit' })}
            >
              Audit log
            </Button>
            <Button
              variant="primary"
              leadingIcon={<LuRocket size={15} />}
              onClick={() => router.navigate({ to: '/deployments/new' })}
            >
              New deployment
            </Button>
          </>
        }
      />

      {stats.isError && (
        <div className="mb-6">
          <Callout variant="danger" title="Fleet stats unavailable">
            We couldn't load the fleet overview. Check that the API is reachable.
          </Callout>
        </div>
      )}

      <div
        className="grid gap-[var(--spacing-card-gap)] mb-[var(--spacing-section)]"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile
          label="Total Machines"
          value={totalMachines ?? '—'}
          icon={<LuMonitor size={18} />}
          tone="ember"
          onClick={() => router.navigate({ to: '/machines' })}
        />
        <MetricTile
          label="Active"
          value={activeMachines ?? '—'}
          sublabel={totalMachines ? `${activityRate}% of fleet` : undefined}
          icon={<LuActivity size={18} />}
          tone="success"
          onClick={() => router.navigate({ to: '/machines' })}
        />
        <MetricTile
          label="Pending Enrollment"
          value={stats.data?.pending_enrollments ?? '—'}
          icon={<LuUserPlus size={18} />}
          tone={stats.data?.pending_enrollments ? 'warning' : 'default'}
          onClick={() => router.navigate({ to: '/enrollment' })}
        />
        <MetricTile
          label="Active Deployments"
          value={stats.data?.active_deployments ?? '—'}
          icon={<LuLayers size={18} />}
          tone="info"
          onClick={() => router.navigate({ to: '/deployments' })}
        />
        <MetricTile
          label="Pending Requests"
          value={stats.data?.pending_requests ?? '—'}
          icon={<LuInbox size={18} />}
          tone={stats.data?.pending_requests ? 'warning' : 'default'}
          onClick={() => router.navigate({ to: '/requests' })}
        />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-[var(--spacing-card-gap)] items-start">
        <Card className="lg:col-span-2">
          <div className="flex items-center justify-between mb-5">
            <div>
              <h2
                className="font-semibold text-[var(--color-text-primary)] text-lg"
               
              >
                Recent Deployments
              </h2>
              <p
                className="text-[var(--color-text-tertiary)] text-xs"
               
              >
                Latest rollouts across the fleet
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              trailingIcon={<LuChevronRight size={14} />}
              onClick={() => router.navigate({ to: '/deployments' })}
            >
              View all
            </Button>
          </div>

          {deployments.isLoading ? (
            <div className="flex flex-col gap-3">
              <SkeletonCard />
              <SkeletonCard />
            </div>
          ) : recent.length === 0 ? (
            <div
              className="text-center py-10 text-[var(--color-text-tertiary)] text-sm"
             
            >
              No deployments yet. Ship one to see it here.
            </div>
          ) : (
            <div className="flex flex-col">
              {recent.map((d) => {
                const progress = d.total_machines ? (d.succeeded / d.total_machines) * 100 : 0;
                return (
                  <button
                    key={d.id}
                    type="button"
                    onClick={() => router.navigate({ to: '/deployments/$deploymentId', params: { deploymentId: d.id } })}
                    className="flex flex-col gap-2 w-full text-left py-4 border-b border-[var(--color-border-subtle)] last:border-b-0 cursor-pointer hover:bg-[var(--color-surface-raised)] transition-colors px-2 -mx-2 rounded-[var(--radius-sm)]"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="flex flex-col gap-0.5 min-w-0">
                        <span
                          className="font-mono text-[var(--color-text-primary)] truncate text-xs"
                         
                          title={d.closure}
                        >
                          {d.closure.split('/').pop() ?? d.closure}
                        </span>
                        <span
                          className="text-[var(--color-text-tertiary)] text-2xs"
                         
                        >
                          {formatRelativeTime(d.updated_at)} · {d.succeeded}/{d.total_machines} machines
                          {d.failed > 0 && (
                            <span className="text-[var(--color-error)]"> · {d.failed} failed</span>
                          )}
                        </span>
                      </div>
                      <StatusChip status={d.status} />
                    </div>
                    <div className="h-1 rounded-full bg-[var(--color-surface-sunken)] overflow-hidden">
                      <div
                        className="h-full rounded-full transition-all"
                        style={{
                          width: `${progress}%`,
                          background: d.status === 'failed' ? 'var(--color-error)' : 'var(--color-ember)',
                        }}
                      />
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </Card>

        <Card>
          <div className="mb-5">
            <h2
              className="font-semibold text-[var(--color-text-primary)] text-lg"
             
            >
              Operations Inbox
            </h2>
            <p
              className="text-[var(--color-text-tertiary)] text-xs"
             
            >
              {operationsInboxCount === 0 ? 'All caught up' : `${operationsInboxCount} items awaiting action`}
            </p>
          </div>

          <div className="flex flex-col gap-3">
            {(pending.data?.length ?? 0) > 0 && (
              <InboxItem
                tone="warning"
                icon={<LuUserPlus size={16} />}
                title={`${pending.data!.length} pending enrollment${pending.data!.length === 1 ? '' : 's'}`}
                subtitle="Awaiting approval"
                onClick={() => router.navigate({ to: '/enrollment' })}
              />
            )}
            {(requests.data?.length ?? 0) > 0 && (
              <InboxItem
                tone="info"
                icon={<LuInbox size={16} />}
                title={`${requests.data!.length} software request${requests.data!.length === 1 ? '' : 's'}`}
                subtitle="Awaiting review"
                onClick={() => router.navigate({ to: '/requests' })}
              />
            )}
            {(stats.data?.active_deployments ?? 0) > 0 && (
              <InboxItem
                tone="info"
                icon={<LuLayers size={16} />}
                title={`${stats.data!.active_deployments} deployment${stats.data!.active_deployments === 1 ? '' : 's'} in flight`}
                subtitle="Monitor progress"
                onClick={() => router.navigate({ to: '/deployments' })}
              />
            )}
            {operationsInboxCount === 0 && (stats.data?.active_deployments ?? 0) === 0 && (
              <div
                className="flex flex-col items-center gap-2 py-8 text-center text-[var(--color-text-tertiary)] text-sm"
               
              >
                <LuShieldAlert size={28} className="text-[var(--color-success)] opacity-50" />
                <span>Nothing needs your attention right now.</span>
              </div>
            )}
          </div>
        </Card>
      </div>

      <div className="mt-[var(--spacing-section)]">
        <Card>
          <div className="mb-5">
            <h2
              className="font-semibold text-[var(--color-text-primary)] text-lg"
             
            >
              Live Fleet Activity
            </h2>
            <p
              className="text-[var(--color-text-tertiary)] text-xs"
             
            >
              Recent deployments and audit events
            </p>
          </div>
          <Timeline events={timelineEvents} emptyLabel="No recent activity" />
        </Card>
      </div>
    </PageContainer>
  );
}

function InboxItem({
  tone,
  icon,
  title,
  subtitle,
  onClick,
}: {
  tone: 'warning' | 'info' | 'success' | 'danger';
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  onClick: () => void;
}) {
  const bg =
    tone === 'warning'
      ? 'var(--color-warning-faint)'
      : tone === 'info'
        ? 'var(--color-info-faint)'
        : tone === 'danger'
          ? 'var(--color-error-faint)'
          : 'var(--color-success-faint)';
  const fg =
    tone === 'warning'
      ? 'var(--color-warning)'
      : tone === 'info'
        ? 'var(--color-info)'
        : tone === 'danger'
          ? 'var(--color-error)'
          : 'var(--color-success)';
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center justify-between gap-3 w-full text-left px-4 py-3 rounded-[var(--radius-sm)] bg-[var(--color-surface-sunken)] border border-[var(--color-border-subtle)] hover:border-[var(--color-border)] hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer"
    >
      <div className="flex items-center gap-3 min-w-0">
        <div
          className="shrink-0 w-9 h-9 rounded-[var(--radius-sm)] flex items-center justify-center"
          style={{ background: bg, color: fg }}
        >
          {icon}
        </div>
        <div className="flex flex-col gap-0.5 min-w-0">
          <span
            className="font-medium text-[var(--color-text-primary)] truncate text-sm"
           
          >
            {title}
          </span>
          <span
            className="text-[var(--color-text-tertiary)] text-2xs"
           
          >
            {subtitle}
          </span>
        </div>
      </div>
      <LuChevronRight size={16} className="shrink-0 text-[var(--color-text-tertiary)]" />
    </button>
  );
}
