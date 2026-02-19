import { useState, useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, FilterPills, Button } from '@hearth/ui';
import { useDeployments } from '../../api/deployments';
import type { Deployment, DeploymentStatus } from '../../api/types';
import { formatRelativeTime, truncateStorePath } from '../../lib/time';
import { LuPlus } from 'react-icons/lu';

const statusColors: Record<DeploymentStatus, string> = {
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  canary: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  rolling: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  completed: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
  rolled_back: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const filterOptions = ['Pending', 'Canary', 'Rolling', 'Completed', 'Failed', 'Rolled Back'];

const columns: ColumnDef<Deployment, unknown>[] = [
  {
    accessorKey: 'closure',
    header: 'Closure',
    cell: ({ row }) => (
      <span className="font-mono text-xs text-[var(--color-text-primary)] truncate max-w-[260px] inline-block">
        {truncateStorePath(row.original.closure)}
      </span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.status;
      const isPulsing = status === 'canary' || status === 'rolling' || status === 'pending';
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${statusColors[status]}`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              status === 'completed'
                ? 'bg-[var(--color-success)]'
                : status === 'failed' || status === 'rolled_back'
                  ? 'bg-[var(--color-error)]'
                  : status === 'pending'
                    ? 'bg-[var(--color-warning)]'
                    : 'bg-[var(--color-info)]'
            } ${isPulsing ? 'animate-[pulse-dot_1.8s_ease-in-out_infinite]' : ''}`}
          />
          {status.replace('_', ' ')}
        </span>
      );
    },
  },
  {
    id: 'progress',
    header: 'Progress',
    cell: ({ row }) => {
      const d = row.original;
      return (
        <span className="text-sm text-[var(--color-text-secondary)]">
          {d.succeeded}/{d.total_machines}
          {d.failed > 0 && (
            <span className="text-[var(--color-error)] ml-1">({d.failed} failed)</span>
          )}
        </span>
      );
    },
  },
  {
    accessorKey: 'created_at',
    header: 'Created',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {formatRelativeTime(row.original.created_at)}
      </span>
    ),
  },
];

export function DeploymentsPage() {
  const router = useRouter();
  const { data: deployments, isLoading } = useDeployments();
  const [activeFilter, setActiveFilter] = useState('All');

  const filtered = useMemo(() => {
    if (!deployments) return [];
    if (activeFilter === 'All') return deployments;
    const filterValue = activeFilter.toLowerCase().replace(' ', '_');
    return deployments.filter((d) => d.status === filterValue);
  }, [deployments, activeFilter]);

  return (
    <div>
      <PageHeader
        title="Deployments"
        description="Manage NixOS closure deployments across the fleet"
        actions={
          <Button
            variant="primary"
            size="sm"
            onClick={() => router.navigate({ to: '/deployments/new' })}
          >
            <LuPlus size={14} />
            New Deployment
          </Button>
        }
      />

      <div className="mb-4">
        <FilterPills
          options={filterOptions}
          active={activeFilter}
          onSelect={setActiveFilter}
        />
      </div>

      {isLoading ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading deployments...
        </p>
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          onRowClick={(deployment) =>
            router.navigate({
              to: '/deployments/$deploymentId',
              params: { deploymentId: deployment.id },
            })
          }
          emptyMessage="No deployments found"
        />
      )}
    </div>
  );
}
