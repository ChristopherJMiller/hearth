import { useMemo, useState } from 'react';
import { useRouter } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  Card,
  StatusChip,
  SegmentedControl,
  Callout,
  SkeletonCard,
  MetricTile,
  EmptyState,
  Tooltip,
} from '@hearth/ui';
import { useBuildJobs } from '../../api/builds';
import type { BuildJob, BuildJobStatus } from '../../api/types';
import { formatRelativeTime, truncateStorePath } from '../../lib/time';
import {
  LuHammer,
  LuClock,
  LuCheckCircle,
  LuXCircle,
  LuLayers,
  LuRocket,
  LuUser,
} from 'react-icons/lu';

type ViewMode = 'kanban' | 'stream';

const stages: BuildJobStatus[] = [
  'pending',
  'claimed',
  'evaluating',
  'building',
  'pushing',
  'deploying',
];

const stageLabel: Record<BuildJobStatus, string> = {
  pending: 'Pending',
  claimed: 'Claimed',
  evaluating: 'Evaluating',
  building: 'Building',
  pushing: 'Pushing',
  deploying: 'Deploying',
  completed: 'Completed',
  failed: 'Failed',
};

function ageMinutes(iso: string): number {
  return (Date.now() - new Date(iso).getTime()) / 1000 / 60;
}

function BuildCard({ job, onClick }: { job: BuildJob; onClick: () => void }) {
  const stuck = ageMinutes(job.updated_at) > 10 && job.status !== 'completed' && job.status !== 'failed';
  return (
    <button
      type="button"
      onClick={onClick}
      className={`group flex flex-col gap-2.5 w-full text-left rounded-[var(--radius-md)] border bg-[var(--color-surface)] cursor-pointer hover:bg-[var(--color-surface-raised)] transition-all p-6 ${
        stuck
          ? 'border-[var(--color-border-accent)] shadow-[0_0_24px_rgba(233,69,96,0.25)]'
          : 'border-[var(--color-border-subtle)]'
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <Tooltip content={job.flake_ref}>
          <span
            className="font-mono text-[var(--color-text-primary)] truncate font-semibold text-xs"
           
          >
            {truncateStorePath(job.flake_ref)}
          </span>
        </Tooltip>
        {stuck && (
          <span
            className="shrink-0 px-1.5 py-0.5 rounded-[6px] bg-[var(--color-error-faint)] text-[var(--color-error)] uppercase font-semibold text-2xs tracking-wide"
           
          >
            Stuck
          </span>
        )}
      </div>
      <div className="flex items-center gap-3 text-[var(--color-text-tertiary)] text-2xs">
        <span className="flex items-center gap-1"><LuClock size={11} />{formatRelativeTime(job.created_at)}</span>
        {job.worker_id && (
          <span className="flex items-center gap-1 truncate">
            <LuUser size={11} />
            <span className="truncate">{job.worker_id}</span>
          </span>
        )}
      </div>
      {job.closure && (
        <Tooltip content={job.closure}>
          <span
            className="font-mono text-[var(--color-text-secondary)] truncate text-2xs"
           
          >
            → {truncateStorePath(job.closure)}
          </span>
        </Tooltip>
      )}
      {job.error_message && (
        <span
          className="text-[var(--color-error)] truncate text-2xs"
         
        >
          {job.error_message}
        </span>
      )}
    </button>
  );
}

export function BuildsPage() {
  const router = useRouter();
  const { data: jobs, isLoading, isError } = useBuildJobs();
  const [view, setView] = useState<ViewMode>('kanban');

  const grouped = useMemo(() => {
    const map = new Map<BuildJobStatus, BuildJob[]>();
    for (const stage of stages) map.set(stage, []);
    for (const job of jobs ?? []) {
      if (map.has(job.status)) map.get(job.status)!.push(job);
    }
    return map;
  }, [jobs]);

  const counts = useMemo(() => {
    const list = jobs ?? [];
    return {
      total: list.length,
      active: list.filter((j) => j.status !== 'completed' && j.status !== 'failed').length,
      completed: list.filter((j) => j.status === 'completed').length,
      failed: list.filter((j) => j.status === 'failed').length,
    };
  }, [jobs]);

  const handleOpen = (job: BuildJob) => {
    router.navigate({ to: '/builds/$jobId', params: { jobId: job.id } });
  };

  const sorted = useMemo(() => {
    return [...(jobs ?? [])].sort((a, b) =>
      b.updated_at.localeCompare(a.updated_at),
    );
  }, [jobs]);

  return (
    <PageContainer size="full">
      <PageHeader
        eyebrow="Software"
        title="Build queue"
        description="The Nix build pipeline — every job from queue to deployment, with stuck-job warnings and worker attribution."
        actions={
          <SegmentedControl
            value={view}
            onChange={setView}
            options={[
              { value: 'kanban', label: 'Pipeline' },
              { value: 'stream', label: 'Stream' },
            ]}
          />
        }
      />

      <div
        className="grid gap-[var(--spacing-card-gap)] mb-[var(--spacing-section)]"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile label="Total" value={counts.total} icon={<LuHammer size={18} />} tone="ember" />
        <MetricTile
          label="In flight"
          value={counts.active}
          icon={<LuLayers size={18} />}
          tone={counts.active > 0 ? 'info' : 'default'}
        />
        <MetricTile label="Completed" value={counts.completed} icon={<LuCheckCircle size={18} />} tone="success" />
        <MetricTile
          label="Failed"
          value={counts.failed}
          icon={<LuXCircle size={18} />}
          tone={counts.failed > 0 ? 'danger' : 'default'}
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load build jobs">
          Verify the build worker queue is reachable. The endpoint{' '}
          <code className="font-mono">/api/v1/build-jobs</code> may not exist yet.
        </Callout>
      ) : isLoading ? (
        <SkeletonCard />
      ) : !jobs || jobs.length === 0 ? (
        <EmptyState
          icon={<LuRocket size={28} />}
          title="No build jobs"
          description="Create a deployment to enqueue a build job. Jobs will flow through the pipeline below."
        />
      ) : view === 'kanban' ? (
        <div className="grid gap-[var(--spacing-card-gap)]" style={{ gridTemplateColumns: `repeat(${stages.length}, minmax(240px, 1fr))` }}>
          {stages.map((stage) => {
            const items = grouped.get(stage) ?? [];
            return (
              <div key={stage} className="flex flex-col gap-3 min-w-0">
                <div className="flex items-center justify-between gap-2 sticky top-0 z-10 px-1 py-2 bg-[var(--color-surface-base)]">
                  <StatusChip status={stage} label={stageLabel[stage]} />
                  <span
                    className="text-[var(--color-text-tertiary)] tabular-nums text-xs"
                   
                  >
                    {items.length}
                  </span>
                </div>
                <div className="flex flex-col gap-2.5">
                  {items.length === 0 ? (
                    <div
                      className="text-center py-6 rounded-[var(--radius-sm)] border border-dashed border-[var(--color-border-subtle)] text-[var(--color-text-tertiary)] text-xs"
                     
                    >
                      Empty
                    </div>
                  ) : (
                    items.map((job) => <BuildCard key={job.id} job={job} onClick={() => handleOpen(job)} />)
                  )}
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <Card>
          <div className="flex flex-col gap-2">
            {sorted.map((job) => (
              <button
                key={job.id}
                type="button"
                onClick={() => handleOpen(job)}
                className="flex items-center gap-4 px-4 py-3 rounded-[var(--radius-sm)] hover:bg-[var(--color-surface-raised)] cursor-pointer text-left"
              >
                <StatusChip status={job.status} />
                <Tooltip content={job.flake_ref}>
                  <span className="font-mono text-[var(--color-text-primary)] truncate flex-1 text-xs">
                    {truncateStorePath(job.flake_ref)}
                  </span>
                </Tooltip>
                <span className="text-[var(--color-text-tertiary)] shrink-0 text-xs">
                  {formatRelativeTime(job.updated_at)}
                </span>
              </button>
            ))}
          </div>
        </Card>
      )}
    </PageContainer>
  );
}
