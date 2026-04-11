import { useState } from 'react';
import { useRouter, useParams } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  ProgressBar,
  ConfirmDialog,
  Button,
  Card,
  StatusChip,
  Tabs,
  Callout,
  DescriptionList,
  Tooltip,
  SkeletonCard,
} from '@hearth/ui';
import {
  useDeployment,
  useDeploymentMachines,
  useRollbackDeployment,
} from '../../api/deployments';
import type { Deployment, DeploymentMachineStatus } from '../../api/types';
import { formatRelativeTime, formatDateTime, truncateStorePath, truncateId } from '../../lib/time';
import {
  LuUndo2,
  LuLayers,
  LuArrowRight,
  LuPackage,
  LuClock,
  LuActivity,
  LuRocket,
} from 'react-icons/lu';

const fsmStages: Array<{ id: Deployment['status']; label: string }> = [
  { id: 'pending', label: 'Pending' },
  { id: 'canary', label: 'Canary' },
  { id: 'rolling', label: 'Rolling' },
  { id: 'completed', label: 'Completed' },
];

function FsmGraph({ status }: { status: Deployment['status'] }) {
  const failed = status === 'failed' || status === 'rolled_back';
  const currentIdx = failed ? -1 : fsmStages.findIndex((s) => s.id === status);

  return (
    <div className="flex items-center gap-2 flex-wrap">
      {fsmStages.map((stage, i) => {
        const isActive = i === currentIdx;
        const isPast = i < currentIdx;
        const tone = isActive
          ? 'var(--color-ember)'
          : isPast
            ? 'var(--color-success)'
            : 'var(--color-text-tertiary)';
        return (
          <div key={stage.id} className="flex items-center gap-2">
            <div
              className="flex items-center gap-2 px-3 py-1.5 rounded-full border text-xs"
             
            >
              <span
                className={`w-2 h-2 rounded-full ${isActive ? 'animate-[pulse-dot_1.8s_ease-in-out_infinite]' : ''}`}
                style={{ background: tone }}
              />
              {stage.label}
            </div>
            {i < fsmStages.length - 1 && (
              <LuArrowRight size={14} className="text-text-tertiary" />
            )}
          </div>
        );
      })}
      {failed && (
        <div className="flex items-center gap-2 ml-2 pl-2 border-l border-border-subtle">
          <StatusChip status={status} />
        </div>
      )}
    </div>
  );
}

const machineColumns: ColumnDef<DeploymentMachineStatus, unknown>[] = [
  {
    accessorKey: 'machine_id',
    header: 'Machine',
    cell: ({ row }) => (
      <span
        className="font-mono text-text-primary text-xs"
       
      >
        {truncateId(row.original.machine_id)}
      </span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => <StatusChip status={row.original.status} />,
  },
  {
    accessorKey: 'started_at',
    header: 'Started',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {row.original.started_at ? formatRelativeTime(row.original.started_at) : <span className="italic text-text-tertiary">—</span>}
      </span>
    ),
  },
  {
    accessorKey: 'completed_at',
    header: 'Completed',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {row.original.completed_at ? formatRelativeTime(row.original.completed_at) : <span className="italic text-text-tertiary">—</span>}
      </span>
    ),
  },
  {
    accessorKey: 'error_message',
    header: 'Error',
    cell: ({ row }) => {
      const err = row.original.error_message;
      if (!err) return <span className="text-text-tertiary">—</span>;
      return (
        <Tooltip content={err}>
          <span
            className="text-error max-w-[280px] truncate inline-block text-xs"
           
          >
            {err}
          </span>
        </Tooltip>
      );
    },
  },
];

export function DeploymentDetailPage() {
  const router = useRouter();
  const { deploymentId } = useParams({ strict: false }) as { deploymentId: string };
  const { data: deployment, isLoading, isError } = useDeployment(deploymentId);
  const { data: machines } = useDeploymentMachines(deploymentId);
  const rollback = useRollbackDeployment();
  const [rollbackOpen, setRollbackOpen] = useState(false);
  const [activeTab, setActiveTab] = useState('overview');

  if (isError) {
    return (
      <PageContainer>
        <Callout variant="danger" title="Deployment not found" />
      </PageContainer>
    );
  }

  if (isLoading || !deployment) {
    return (
      <PageContainer>
        <PageHeader title="Loading deployment…" />
        <SkeletonCard />
      </PageContainer>
    );
  }

  const canRollback = deployment.status === 'canary' || deployment.status === 'rolling';
  const progressVariant =
    deployment.status === 'failed' || deployment.status === 'rolled_back'
      ? 'error'
      : deployment.status === 'completed'
        ? 'success'
        : 'default';

  const tabs = [
    { id: 'overview', label: 'Overview' },
    { id: 'machines', label: 'Machines', count: machines?.length },
  ];

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow={truncateId(deployment.id)}
        title={truncateStorePath(deployment.closure)}
        description={`Created ${formatRelativeTime(deployment.created_at)} · Updated ${formatRelativeTime(deployment.updated_at)}`}
        breadcrumbs={[
          { label: 'Software' },
          { label: 'Deployments', onClick: () => router.navigate({ to: '/deployments' }) },
          { label: truncateId(deployment.id) },
        ]}
        actions={
          canRollback ? (
            <Button
              variant="danger"
              leadingIcon={<LuUndo2 size={14} />}
              onClick={() => setRollbackOpen(true)}
              disabled={rollback.isPending}
            >
              Rollback
            </Button>
          ) : undefined
        }
      />

      <ConfirmDialog
        open={rollbackOpen}
        onOpenChange={setRollbackOpen}
        title="Rollback deployment"
        description="This halts the current rollout and reverts affected machines to their previous closure. This cannot be undone."
        confirmLabel="Rollback"
        variant="danger"
        onConfirm={() => rollback.mutate(deployment.id)}
      />

      <Card className="mb-card-gap">
        <div className="flex items-start justify-between gap-6 flex-wrap">
          <div className="flex flex-col gap-3">
            <span
              className="uppercase font-semibold text-text-tertiary text-2xs tracking-wide"
             
            >
              Pipeline state
            </span>
            <FsmGraph status={deployment.status} />
          </div>
          <div className="min-w-[280px] flex-1">
            <ProgressBar
              value={deployment.succeeded}
              max={deployment.total_machines || 1}
              label={`${deployment.succeeded} of ${deployment.total_machines} machines succeeded${deployment.failed > 0 ? ` · ${deployment.failed} failed` : ''}`}
              variant={progressVariant}
            />
          </div>
        </div>

        {deployment.rollback_reason && (
          <div className="mt-4">
            <Callout variant="danger" title="Rolled back">
              {deployment.rollback_reason}
            </Callout>
          </div>
        )}
      </Card>

      <Tabs tabs={tabs} activeId={activeTab} onChange={setActiveTab} />

      <div className="mt-6">
        {activeTab === 'overview' && (
          <Card>
            <div className="flex items-center gap-2 mb-5">
              <LuPackage size={16} className="text-text-tertiary" />
              <h2
                className="font-semibold text-text-primary text-lg"
               
              >
                Configuration
              </h2>
            </div>
            <DescriptionList
              columns={3}
              items={[
                { label: 'Canary size', icon: <LuRocket size={12} />, value: `${deployment.canary_size} machine${deployment.canary_size === 1 ? '' : 's'}` },
                { label: 'Batch size', icon: <LuLayers size={12} />, value: `${deployment.batch_size} machine${deployment.batch_size === 1 ? '' : 's'}` },
                { label: 'Failure threshold', icon: <LuActivity size={12} />, value: `${(deployment.failure_threshold * 100).toFixed(0)}%` },
                { label: 'Total machines', icon: <LuLayers size={12} />, value: deployment.total_machines },
                { label: 'Created', icon: <LuClock size={12} />, value: formatDateTime(deployment.created_at) },
                { label: 'Updated', icon: <LuClock size={12} />, value: formatDateTime(deployment.updated_at) },
                {
                  label: 'Closure',
                  icon: <LuPackage size={12} />,
                  value: <span className="font-mono break-all text-xs">{deployment.closure}</span>,
                  span: 3,
                },
              ]}
            />
          </Card>
        )}

        {activeTab === 'machines' && (
          <Card>
            <DataTable
              data={machines ?? []}
              columns={machineColumns}
              emptyMessage="No machine records for this deployment"
              density="comfortable"
              pageSize={50}
            />
          </Card>
        )}
      </div>
    </PageContainer>
  );
}
