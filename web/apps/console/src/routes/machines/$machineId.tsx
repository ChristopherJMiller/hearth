import { useState } from 'react';
import { useRouter, useParams } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, Tabs } from '@hearth/ui';
import { useMachine } from '../../api/machines';
import { useMachineEnvironments } from '../../api/environments';
import type { UserEnvironment } from '../../api/types';
import { formatRelativeTime, formatDateTime } from '../../lib/time';
import {
  LuMonitor,
  LuShield,
  LuClock,
  LuTag,
  LuFingerprint,
  LuBox,
  LuTarget,
} from 'react-icons/lu';

const enrollmentColors: Record<string, string> = {
  active: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  enrolled: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  approved: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  provisioning: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  decommissioned: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const envStatusColors: Record<string, string> = {
  active: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  ready: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  building: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  activating: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const tabs = [
  { id: 'overview', label: 'Overview' },
  { id: 'environments', label: 'Environments' },
  { id: 'audit', label: 'Audit' },
];

const envColumns: ColumnDef<UserEnvironment, unknown>[] = [
  {
    accessorKey: 'username',
    header: 'Username',
    cell: ({ row }) => (
      <span className="font-medium">{row.original.username}</span>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.role}
      </span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.status;
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${envStatusColors[status] ?? ''}`}
        >
          {status}
        </span>
      );
    },
  },
  {
    accessorKey: 'updated_at',
    header: 'Updated',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {formatRelativeTime(row.original.updated_at)}
      </span>
    ),
  },
];

function InfoField({
  icon,
  label,
  value,
  mono,
}: {
  icon: React.ReactNode;
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex items-start gap-3 p-4 bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)]">
      <div className="w-8 h-8 rounded-[var(--radius-sm)] bg-[var(--color-surface-raised)] flex items-center justify-center text-[var(--color-text-tertiary)] shrink-0">
        {icon}
      </div>
      <div className="min-w-0">
        <p className="text-xs text-[var(--color-text-tertiary)] mb-0.5">{label}</p>
        <div className={`text-sm text-[var(--color-text-primary)] break-all ${mono ? 'font-mono text-xs' : ''}`}>
          {value}
        </div>
      </div>
    </div>
  );
}

export function MachineDetailPage() {
  const router = useRouter();
  const { machineId } = useParams({ strict: false }) as { machineId: string };
  const { data: machine, isLoading } = useMachine(machineId);
  const { data: environments } = useMachineEnvironments(machineId);
  const [activeTab, setActiveTab] = useState('overview');

  if (isLoading || !machine) {
    return (
      <div>
        <PageHeader
          title="Loading..."
          breadcrumbs={[
            { label: 'Machines', onClick: () => router.navigate({ to: '/machines' }) },
            { label: '...' },
          ]}
        />
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading machine details...
        </p>
      </div>
    );
  }

  const status = machine.enrollment_status;

  return (
    <div>
      <PageHeader
        title={machine.hostname}
        description={`Machine ID: ${machine.id}`}
        breadcrumbs={[
          { label: 'Machines', onClick: () => router.navigate({ to: '/machines' }) },
          { label: machine.hostname },
        ]}
        actions={
          <span
            className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${enrollmentColors[status] ?? ''}`}
          >
            {status}
          </span>
        }
      />

      <Tabs tabs={tabs} activeId={activeTab} onChange={setActiveTab} />

      <div className="mt-6">
        {activeTab === 'overview' && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            <InfoField
              icon={<LuMonitor size={16} />}
              label="Hostname"
              value={machine.hostname}
            />
            <InfoField
              icon={<LuShield size={16} />}
              label="Role"
              value={machine.role ?? 'Not assigned'}
            />
            <InfoField
              icon={<LuClock size={16} />}
              label="Last Heartbeat"
              value={
                machine.last_heartbeat
                  ? formatRelativeTime(machine.last_heartbeat)
                  : 'Never'
              }
            />
            <InfoField
              icon={<LuBox size={16} />}
              label="Current Closure"
              value={machine.current_closure ?? 'None'}
              mono
            />
            <InfoField
              icon={<LuTarget size={16} />}
              label="Target Closure"
              value={machine.target_closure ?? 'None'}
              mono
            />
            <InfoField
              icon={<LuFingerprint size={16} />}
              label="Hardware Fingerprint"
              value={machine.hardware_fingerprint ?? 'Unknown'}
              mono
            />
            <InfoField
              icon={<LuTag size={16} />}
              label="Tags"
              value={
                machine.tags.length === 0 ? (
                  'None'
                ) : (
                  <div className="flex flex-wrap gap-1 mt-0.5">
                    {machine.tags.map((tag) => (
                      <span
                        key={tag}
                        className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)]"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                )
              }
            />
            <InfoField
              icon={<LuClock size={16} />}
              label="Created"
              value={formatDateTime(machine.created_at)}
            />
            <InfoField
              icon={<LuClock size={16} />}
              label="Updated"
              value={formatDateTime(machine.updated_at)}
            />
          </div>
        )}

        {activeTab === 'environments' && (
          <DataTable
            data={environments ?? []}
            columns={envColumns}
            emptyMessage="No user environments found for this machine"
          />
        )}

        {activeTab === 'audit' && (
          <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
            View the{' '}
            <button
              type="button"
              onClick={() => router.navigate({ to: '/audit' })}
              className="text-[var(--color-ember)] hover:underline cursor-pointer"
            >
              audit log
            </button>{' '}
            filtered for this machine.
          </p>
        )}
      </div>
    </div>
  );
}
