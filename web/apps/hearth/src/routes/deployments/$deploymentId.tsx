import { useState } from 'react';
import { useRouter, useParams } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageHeader,
  DataTable,
  ProgressBar,
  ConfirmDialog,
  Button,
  Card,
} from '@hearth/ui';
import {
  useDeployment,
  useDeploymentMachines,
  useRollbackDeployment,
} from '../../api/deployments';
import type { DeploymentMachineStatus, DeploymentStatus, MachineUpdateStatus } from '../../api/types';
import { formatRelativeTime, formatDateTime, truncateStorePath, truncateId } from '../../lib/time';
import { LuUndo2, LuSettings, LuLayers } from 'react-icons/lu';

const deployStatusColors: Record<DeploymentStatus, string> = {
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  canary: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  rolling: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  completed: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
  rolled_back: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const machineStatusColors: Record<MachineUpdateStatus, string> = {
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  downloading: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  switching: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  completed: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
  rolled_back: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const machineColumns: ColumnDef<DeploymentMachineStatus, unknown>[] = [
  {
    accessorKey: 'machine_id',
    header: 'Machine ID',
    cell: ({ row }) => (
      <span className="font-mono text-xs">{truncateId(row.original.machine_id)}</span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.status;
      const isPulsing = status === 'downloading' || status === 'switching' || status === 'pending';
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${machineStatusColors[status]}`}
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
    accessorKey: 'started_at',
    header: 'Started',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.started_at ? formatRelativeTime(row.original.started_at) : '—'}
      </span>
    ),
  },
  {
    accessorKey: 'completed_at',
    header: 'Completed',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.completed_at ? formatRelativeTime(row.original.completed_at) : '—'}
      </span>
    ),
  },
  {
    accessorKey: 'error_message',
    header: 'Error',
    cell: ({ row }) => {
      const err = row.original.error_message;
      if (!err) return <span className="text-[var(--color-text-tertiary)]">—</span>;
      return (
        <span className="text-xs text-[var(--color-error)] max-w-[200px] inline-block truncate" title={err}>
          {err}
        </span>
      );
    },
  },
];

function ConfigItem({ label, value }: { label: string; value: string | number }) {
  return (
    <div>
      <p className="text-xs text-[var(--color-text-tertiary)] mb-0.5">{label}</p>
      <p className="text-sm text-[var(--color-text-primary)] font-mono">{value}</p>
    </div>
  );
}

export function DeploymentDetailPage() {
  const router = useRouter();
  const { deploymentId } = useParams({ strict: false }) as { deploymentId: string };
  const { data: deployment, isLoading } = useDeployment(deploymentId);
  const { data: machines } = useDeploymentMachines(deploymentId);
  const rollback = useRollbackDeployment();
  const [rollbackOpen, setRollbackOpen] = useState(false);

  if (isLoading || !deployment) {
    return (
      <div>
        <PageHeader
          title="Loading..."
          breadcrumbs={[
            { label: 'Deployments', onClick: () => router.navigate({ to: '/deployments' }) },
            { label: '...' },
          ]}
        />
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading deployment details...
        </p>
      </div>
    );
  }

  const canRollback = deployment.status === 'canary' || deployment.status === 'rolling';
  const progressVariant =
    deployment.status === 'failed' || deployment.status === 'rolled_back'
      ? 'error'
      : deployment.status === 'completed'
        ? 'success'
        : 'default';

  return (
    <div>
      <PageHeader
        title={truncateStorePath(deployment.closure)}
        description={`Deployment ${truncateId(deployment.id)}`}
        breadcrumbs={[
          { label: 'Deployments', onClick: () => router.navigate({ to: '/deployments' }) },
          { label: truncateId(deployment.id) },
        ]}
        actions={
          <div className="flex items-center gap-3">
            <span
              className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${deployStatusColors[deployment.status]}`}
            >
              {deployment.status.replace('_', ' ')}
            </span>
            {canRollback && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => setRollbackOpen(true)}
                disabled={rollback.isPending}
              >
                <LuUndo2 size={14} />
                Rollback
              </Button>
            )}
          </div>
        }
      />

      <ConfirmDialog
        open={rollbackOpen}
        onOpenChange={setRollbackOpen}
        title="Rollback Deployment"
        description="This will halt the current rollout and revert all affected machines to their previous closure. Are you sure you want to proceed?"
        confirmLabel="Rollback"
        variant="danger"
        onConfirm={() => rollback.mutate(deployment.id)}
      />

      {/* Progress */}
      <div className="mb-6">
        <ProgressBar
          value={deployment.succeeded}
          max={deployment.total_machines || 1}
          label={`${deployment.succeeded} of ${deployment.total_machines} machines succeeded`}
          variant={progressVariant}
        />
      </div>

      {/* Config */}
      <Card className="mb-6">
        <div className="flex items-center gap-2 mb-4">
          <LuSettings size={16} className="text-[var(--color-text-tertiary)]" />
          <h2 className="text-sm font-semibold text-[var(--color-text-primary)]">Configuration</h2>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-5">
          <ConfigItem label="Canary Size" value={deployment.canary_size} />
          <ConfigItem label="Batch Size" value={deployment.batch_size} />
          <ConfigItem label="Failure Threshold" value={`${(deployment.failure_threshold * 100).toFixed(0)}%`} />
          <ConfigItem label="Created" value={formatDateTime(deployment.created_at)} />
          <ConfigItem label="Updated" value={formatDateTime(deployment.updated_at)} />
        </div>
        {deployment.rollback_reason && (
          <div className="mt-4 p-3 bg-[var(--color-error-faint)] border border-[var(--color-error)] rounded-[var(--radius-sm)]">
            <p className="text-xs font-medium text-[var(--color-error)]">Rollback Reason</p>
            <p className="text-sm text-[var(--color-text-primary)] mt-1">{deployment.rollback_reason}</p>
          </div>
        )}
      </Card>

      {/* Machine statuses */}
      <div className="flex items-center gap-2 mb-3">
        <LuLayers size={16} className="text-[var(--color-text-tertiary)]" />
        <h2 className="text-sm font-semibold text-[var(--color-text-primary)]">Machine Status</h2>
      </div>
      <DataTable
        data={machines ?? []}
        columns={machineColumns}
        emptyMessage="No machine records for this deployment"
      />
    </div>
  );
}
