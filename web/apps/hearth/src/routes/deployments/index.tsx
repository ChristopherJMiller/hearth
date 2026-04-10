import { useState, useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  Button,
  StatusChip,
  SegmentedControl,
  Tooltip,
  ProgressBar,
  Callout,
  SkeletonTable,
  MetricTile,
} from '@hearth/ui';
import { useDeployments } from '../../api/deployments';
import type { Deployment } from '../../api/types';
import { formatRelativeTime, truncateStorePath } from '../../lib/time';
import { LuPlus, LuLayers, LuCheckCircle, LuXCircle, LuRefreshCw } from 'react-icons/lu';

type StatusFilter = 'all' | 'active' | 'completed' | 'failed';

const inFlight = (s: Deployment['status']) =>
  s === 'pending' || s === 'canary' || s === 'rolling';

const columns: ColumnDef<Deployment, unknown>[] = [
  {
    accessorKey: 'closure',
    header: 'Closure',
    cell: ({ row }) => (
      <Tooltip content={row.original.closure} side="top">
        <span
          className="font-mono text-[var(--color-text-primary)] text-xs"
         
        >
          {truncateStorePath(row.original.closure)}
        </span>
      </Tooltip>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => <StatusChip status={row.original.status} />,
  },
  {
    id: 'progress',
    header: 'Progress',
    cell: ({ row }) => {
      const d = row.original;
      const pct = d.total_machines ? (d.succeeded / d.total_machines) * 100 : 0;
      return (
        <div className="flex flex-col gap-1.5 min-w-[160px]">
          <div className="flex items-baseline justify-between gap-2">
            <span
              className="text-[var(--color-text-secondary)] tabular-nums text-xs"
             
            >
              {d.succeeded}/{d.total_machines}
            </span>
            {d.failed > 0 && (
              <span
                className="text-[var(--color-error)] text-2xs"
               
              >
                {d.failed} failed
              </span>
            )}
          </div>
          <ProgressBar
            value={pct}
            variant={d.failed > 0 ? 'error' : d.status === 'completed' ? 'success' : 'default'}
            size="sm"
          />
        </div>
      );
    },
  },
  {
    id: 'fleet',
    header: 'Fleet',
    cell: ({ row }) => (
      <span className="text-[var(--color-text-secondary)] text-xs">
        canary {row.original.canary_size} · batch {row.original.batch_size}
      </span>
    ),
  },
  {
    accessorKey: 'created_at',
    header: 'Created',
    cell: ({ row }) => (
      <span className="text-[var(--color-text-secondary)] text-xs">
        {formatRelativeTime(row.original.created_at)}
      </span>
    ),
  },
];

export function DeploymentsPage() {
  const router = useRouter();
  const { data: deployments, isLoading, isError } = useDeployments();
  const [filter, setFilter] = useState<StatusFilter>('all');

  const counts = useMemo(() => {
    const list = deployments ?? [];
    return {
      total: list.length,
      active: list.filter((d) => inFlight(d.status)).length,
      completed: list.filter((d) => d.status === 'completed').length,
      failed: list.filter((d) => d.status === 'failed' || d.status === 'rolled_back').length,
    };
  }, [deployments]);

  const filtered = useMemo(() => {
    const list = deployments ?? [];
    if (filter === 'all') return list;
    if (filter === 'active') return list.filter((d) => inFlight(d.status));
    if (filter === 'completed') return list.filter((d) => d.status === 'completed');
    if (filter === 'failed') return list.filter((d) => d.status === 'failed' || d.status === 'rolled_back');
    return list;
  }, [deployments, filter]);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Software"
        title="Deployments"
        description="Roll out NixOS closures across the fleet — canary, batch, observe, and recover."
        actions={
          <Button
            variant="primary"
            leadingIcon={<LuPlus size={15} />}
            onClick={() => router.navigate({ to: '/deployments/new' })}
          >
            New deployment
          </Button>
        }
      />

      <div
        className="grid gap-[var(--spacing-card-gap)] mb-[var(--spacing-section)]"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile label="Total" value={counts.total} icon={<LuLayers size={18} />} tone="ember" />
        <MetricTile label="In flight" value={counts.active} icon={<LuRefreshCw size={18} />} tone={counts.active > 0 ? 'info' : 'default'} />
        <MetricTile label="Completed" value={counts.completed} icon={<LuCheckCircle size={18} />} tone="success" />
        <MetricTile label="Failed / rolled back" value={counts.failed} icon={<LuXCircle size={18} />} tone={counts.failed > 0 ? 'danger' : 'default'} />
      </div>

      <div className="mb-4">
        <SegmentedControl
          value={filter}
          onChange={setFilter}
          options={[
            { value: 'all', label: `All · ${counts.total}` },
            { value: 'active', label: `Active · ${counts.active}` },
            { value: 'completed', label: `Completed · ${counts.completed}` },
            { value: 'failed', label: `Failed · ${counts.failed}` },
          ]}
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load deployments">
          Verify the control plane is reachable.
        </Callout>
      ) : isLoading ? (
        <SkeletonTable rows={6} cols={5} />
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          onRowClick={(d: Deployment) =>
            router.navigate({
              to: '/deployments/$deploymentId',
              params: { deploymentId: d.id },
            })
          }
          emptyMessage="No deployments match your filter"
          density="comfortable"
          pageSize={20}
        />
      )}
    </PageContainer>
  );
}
