import { useState } from 'react';
import { useRouter, useParams } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  Tabs,
  Card,
  StatusChip,
  DescriptionList,
  DataTable,
  Callout,
  Button,
  SkeletonCard,
  Tooltip,
} from '@hearth/ui';
import { useMachine } from '../../api/machines';
import { useMachineEnvironments } from '../../api/environments';
import { useAuditLog } from '../../api/audit';
import type { UserEnvironment, AuditEvent } from '../../api/types';
import { formatRelativeTime, formatDateTime, truncateId } from '../../lib/time';
import { MachineActions } from '../../components/MachineActions';
import {
  LuMonitor,
  LuShield,
  LuClock,
  LuTag,
  LuFingerprint,
  LuBox,
  LuTarget,
  LuNetwork,
  LuCpu,
  LuCopy,
  LuArrowLeft,
} from 'react-icons/lu';

const tabs = [
  { id: 'overview', label: 'Overview' },
  { id: 'environments', label: 'Environments' },
  { id: 'actions', label: 'Actions' },
  { id: 'audit', label: 'Audit' },
];

const envColumns: ColumnDef<UserEnvironment, unknown>[] = [
  {
    accessorKey: 'username',
    header: 'Username',
    cell: ({ row }) => (
      <span className="font-semibold text-text-primary">{row.original.username}</span>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ row }) => (
      <span className="text-text-secondary capitalize">{row.original.role}</span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => <StatusChip status={row.original.status} />,
  },
  {
    accessorKey: 'updated_at',
    header: 'Updated',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {formatRelativeTime(row.original.updated_at)}
      </span>
    ),
  },
];

const auditColumns: ColumnDef<AuditEvent, unknown>[] = [
  {
    accessorKey: 'event_type',
    header: 'Event',
    cell: ({ row }) => (
      <span className="font-medium text-text-primary">
        {row.original.event_type}
      </span>
    ),
  },
  {
    accessorKey: 'actor',
    header: 'Actor',
    cell: ({ row }) => (
      <span className="text-text-secondary text-sm">
        {row.original.actor ?? <span className="italic text-text-tertiary">system</span>}
      </span>
    ),
  },
  {
    accessorKey: 'created_at',
    header: 'When',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {formatRelativeTime(row.original.created_at)}
      </span>
    ),
  },
];

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Tooltip content={copied ? 'Copied!' : 'Copy'}>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          navigator.clipboard.writeText(value);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="inline-flex items-center justify-center w-6 h-6 rounded-[6px] text-text-tertiary hover:text-text-primary hover:bg-surface-raised cursor-pointer"
        aria-label="Copy"
      >
        <LuCopy size={12} />
      </button>
    </Tooltip>
  );
}

function MonoValue({ value }: { value: string }) {
  return (
    <span className="inline-flex items-center gap-2 max-w-full">
      <span
        className="font-mono break-all text-text-primary text-xs"
       
      >
        {value}
      </span>
      <CopyButton value={value} />
    </span>
  );
}

export function MachineDetailPage() {
  const router = useRouter();
  const { machineId } = useParams({ strict: false }) as { machineId: string };
  const { data: machine, isLoading, isError } = useMachine(machineId);
  const { data: environments } = useMachineEnvironments(machineId);
  const { data: auditEvents } = useAuditLog({ machine_id: machineId, limit: 100 });
  const [activeTab, setActiveTab] = useState('overview');

  if (isError) {
    return (
      <PageContainer>
        <Callout variant="danger" title="Machine not found">
          We couldn't load this machine. It may have been decommissioned.
          <div className="mt-3">
            <Button variant="subtle" leadingIcon={<LuArrowLeft size={14} />} onClick={() => router.navigate({ to: '/machines' })}>
              Back to machines
            </Button>
          </div>
        </Callout>
      </PageContainer>
    );
  }

  if (isLoading || !machine) {
    return (
      <PageContainer>
        <PageHeader
          title="Loading machine…"
          breadcrumbs={[
            { label: 'Machines', onClick: () => router.navigate({ to: '/machines' }) },
            { label: '—' },
          ]}
        />
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-card-gap items-start">
          <div className="lg:col-span-2"><SkeletonCard /></div>
          <SkeletonCard />
        </div>
      </PageContainer>
    );
  }

  const overviewItems = [
    {
      label: 'Hostname',
      icon: <LuMonitor size={12} />,
      value: machine.hostname,
    },
    {
      label: 'Role',
      icon: <LuShield size={12} />,
      value: <span className="capitalize">{machine.role ?? <span className="italic text-text-tertiary">unassigned</span>}</span>,
    },
    {
      label: 'Last heartbeat',
      icon: <LuClock size={12} />,
      value: machine.last_heartbeat
        ? formatRelativeTime(machine.last_heartbeat)
        : <span className="italic text-text-tertiary">never</span>,
    },
    {
      label: 'Mesh address',
      icon: <LuNetwork size={12} />,
      value: machine.headscale_ip
        ? <MonoValue value={machine.headscale_ip} />
        : <span className="italic text-text-tertiary">not connected</span>,
    },
    {
      label: 'Hardware fingerprint',
      icon: <LuFingerprint size={12} />,
      value: machine.hardware_fingerprint
        ? <MonoValue value={machine.hardware_fingerprint} />
        : <span className="italic text-text-tertiary">unknown</span>,
      span: 2 as const,
    },
    {
      label: 'Current closure',
      icon: <LuBox size={12} />,
      value: machine.current_closure
        ? <MonoValue value={machine.current_closure} />
        : <span className="italic text-text-tertiary">none</span>,
      span: 2 as const,
    },
    {
      label: 'Target closure',
      icon: <LuTarget size={12} />,
      value: machine.target_closure
        ? <MonoValue value={machine.target_closure} />
        : <span className="italic text-text-tertiary">none</span>,
      span: 2 as const,
    },
    {
      label: 'Tags',
      icon: <LuTag size={12} />,
      value:
        machine.tags.length === 0 ? (
          <span className="italic text-text-tertiary">none</span>
        ) : (
          <div className="flex flex-wrap gap-1.5">
            {machine.tags.map((tag) => (
              <span
                key={tag}
                className="font-mono px-2 py-0.5 rounded-[6px] bg-surface-sunken text-text-secondary border border-border-subtle text-2xs"
               
              >
                {tag}
              </span>
            ))}
          </div>
        ),
      span: 2 as const,
    },
    {
      label: 'Created',
      icon: <LuClock size={12} />,
      value: formatDateTime(machine.created_at),
    },
    {
      label: 'Updated',
      icon: <LuClock size={12} />,
      value: formatDateTime(machine.updated_at),
    },
  ];

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow={truncateId(machine.id)}
        title={machine.hostname}
        description={machine.role ? `Role · ${machine.role}` : 'No role assigned'}
        breadcrumbs={[
          { label: 'Fleet' },
          { label: 'Machines', onClick: () => router.navigate({ to: '/machines' }) },
          { label: machine.hostname },
        ]}
        actions={<StatusChip status={machine.enrollment_status} size="md" />}
      />

      <Tabs tabs={tabs} activeId={activeTab} onChange={setActiveTab} />

      <div className="mt-6">
        {activeTab === 'overview' && (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-card-gap items-start">
            <Card className="lg:col-span-2">
              <div className="flex items-center gap-2 mb-5">
                <LuCpu size={16} className="text-text-tertiary" />
                <h2
                  className="font-semibold text-text-primary text-lg"
                 
                >
                  Identity & state
                </h2>
              </div>
              <DescriptionList items={overviewItems} columns={2} />
            </Card>

            <Card>
              <div className="mb-4">
                <h2
                  className="font-semibold text-text-primary text-lg"
                 
                >
                  Remote actions
                </h2>
                <p className="text-text-tertiary text-xs">
                  Dispatch commands to this machine
                </p>
              </div>
              <MachineActions machineId={machineId} />
            </Card>
          </div>
        )}

        {activeTab === 'environments' && (
          <Card>
            <DataTable
              data={environments ?? []}
              columns={envColumns}
              emptyMessage="No user environments on this machine yet"
              density="comfortable"
            />
          </Card>
        )}

        {activeTab === 'actions' && (
          <Card>
            <MachineActions machineId={machineId} />
          </Card>
        )}

        {activeTab === 'audit' && (
          <Card>
            <DataTable
              data={auditEvents ?? []}
              columns={auditColumns}
              emptyMessage="No audit events for this machine"
              density="comfortable"
              pageSize={25}
            />
          </Card>
        )}
      </div>
    </PageContainer>
  );
}
