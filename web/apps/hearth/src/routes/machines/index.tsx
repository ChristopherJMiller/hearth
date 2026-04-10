import { useState, useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  SearchInput,
  StatusChip,
  SegmentedControl,
  Button,
  SkeletonTable,
  Callout,
  Tooltip,
  MetricTile,
} from '@hearth/ui';
import { useMachines } from '../../api/machines';
import type { Machine, EnrollmentStatus } from '../../api/types';
import { LuMonitor, LuActivity, LuTag, LuClock } from 'react-icons/lu';
import { formatRelativeTime } from '../../lib/time';

type StatusFilter = 'all' | 'active' | 'pending' | 'decommissioned';

const columns: ColumnDef<Machine, unknown>[] = [
  {
    accessorKey: 'hostname',
    header: 'Hostname',
    cell: ({ row }) => (
      <div className="flex items-center gap-2.5 min-w-0">
        <div className="shrink-0 w-8 h-8 rounded-[var(--radius-sm)] bg-[var(--color-surface-raised)] border border-[var(--color-border-subtle)] flex items-center justify-center text-[var(--color-text-secondary)]">
          <LuMonitor size={14} />
        </div>
        <div className="flex flex-col gap-0.5 min-w-0">
          <span
            className="font-semibold text-[var(--color-text-primary)] truncate text-sm"
           
          >
            {row.original.hostname}
          </span>
          {row.original.headscale_ip && (
            <span
              className="font-mono text-[var(--color-text-tertiary)] text-2xs"
             
            >
              {row.original.headscale_ip}
            </span>
          )}
        </div>
      </div>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ row }) => (
      <span
        className="text-[var(--color-text-secondary)] capitalize text-sm"
       
      >
        {row.original.role ?? '—'}
      </span>
    ),
  },
  {
    accessorKey: 'enrollment_status',
    header: 'Status',
    cell: ({ row }) => <StatusChip status={row.original.enrollment_status} />,
  },
  {
    accessorKey: 'last_heartbeat',
    header: 'Last seen',
    cell: ({ row }) => (
      <span
        className="text-[var(--color-text-secondary)] text-xs"
       
      >
        {row.original.last_heartbeat
          ? formatRelativeTime(row.original.last_heartbeat)
          : <span className="text-[var(--color-text-tertiary)] italic">never</span>}
      </span>
    ),
  },
  {
    accessorKey: 'tags',
    header: 'Tags',
    enableSorting: false,
    cell: ({ row }) => {
      if (row.original.tags.length === 0) {
        return <span className="text-[var(--color-text-tertiary)] text-xs">—</span>;
      }
      const visible = row.original.tags.slice(0, 3);
      const more = row.original.tags.length - visible.length;
      return (
        <div className="flex flex-wrap gap-1.5 max-w-[280px]">
          {visible.map((tag) => (
            <span
              key={tag}
              className="font-mono px-2 py-0.5 rounded-[6px] bg-[var(--color-surface-sunken)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)] text-2xs"
             
            >
              {tag}
            </span>
          ))}
          {more > 0 && (
            <Tooltip content={row.original.tags.slice(3).join(', ')}>
              <span
                className="font-mono px-2 py-0.5 rounded-[6px] bg-[var(--color-surface-raised)] text-[var(--color-text-tertiary)] border border-[var(--color-border-subtle)] text-2xs"
               
              >
                +{more}
              </span>
            </Tooltip>
          )}
        </div>
      );
    },
  },
];

function isActive(status: EnrollmentStatus) {
  return status === 'active' || status === 'enrolled';
}

export function MachinesPage() {
  const router = useRouter();
  const { data: machines, isLoading, isError } = useMachines();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');

  const filtered = useMemo(() => {
    if (!machines) return [];
    let result = machines;
    if (statusFilter === 'active') {
      result = result.filter((m) => isActive(m.enrollment_status));
    } else if (statusFilter === 'pending') {
      result = result.filter((m) =>
        ['pending', 'approved', 'provisioning'].includes(m.enrollment_status),
      );
    } else if (statusFilter === 'decommissioned') {
      result = result.filter((m) => m.enrollment_status === 'decommissioned');
    }
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(
        (m) =>
          m.hostname.toLowerCase().includes(q) ||
          (m.role ?? '').toLowerCase().includes(q) ||
          m.enrollment_status.toLowerCase().includes(q) ||
          m.tags.some((t) => t.toLowerCase().includes(q)),
      );
    }
    return result;
  }, [machines, search, statusFilter]);

  const counts = useMemo(() => {
    const total = machines?.length ?? 0;
    const active = machines?.filter((m) => isActive(m.enrollment_status)).length ?? 0;
    const pending =
      machines?.filter((m) => ['pending', 'approved', 'provisioning'].includes(m.enrollment_status)).length ?? 0;
    const tagged = machines?.filter((m) => m.tags.length > 0).length ?? 0;
    return { total, active, pending, tagged };
  }, [machines]);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Fleet"
        title="Machines"
        description="Every device enrolled in the fleet — searchable, filterable, and one click away from deep diagnostics."
        actions={
          <Button
            variant="primary"
            leadingIcon={<LuMonitor size={15} />}
            onClick={() => router.navigate({ to: '/enrollment' })}
          >
            Pending enrollment
          </Button>
        }
      />

      <div
        className="grid gap-[var(--spacing-card-gap)] mb-[var(--spacing-section)]"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile label="Fleet size" value={counts.total} icon={<LuMonitor size={18} />} tone="ember" />
        <MetricTile label="Active" value={counts.active} icon={<LuActivity size={18} />} tone="success" />
        <MetricTile
          label="Provisioning"
          value={counts.pending}
          icon={<LuClock size={18} />}
          tone={counts.pending > 0 ? 'warning' : 'default'}
        />
        <MetricTile label="Tagged" value={counts.tagged} icon={<LuTag size={18} />} tone="info" />
      </div>

      <div className="flex items-center justify-between gap-3 flex-wrap mb-4">
        <div className="flex items-center gap-3 flex-wrap">
          <SegmentedControl
            value={statusFilter}
            onChange={setStatusFilter}
            options={[
              { value: 'all', label: `All ${counts.total > 0 ? `· ${counts.total}` : ''}` },
              { value: 'active', label: `Active · ${counts.active}` },
              { value: 'pending', label: `Pending · ${counts.pending}` },
              { value: 'decommissioned', label: 'Decommissioned' },
            ]}
          />
        </div>
        <div className="min-w-[280px] flex-1 max-w-md">
          <SearchInput
            value={search}
            onChange={setSearch}
            placeholder="Search by hostname, role, status, or tag…"
          />
        </div>
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load machines">
          The API returned an error. Verify that the control plane is reachable.
        </Callout>
      ) : isLoading ? (
        <SkeletonTable rows={8} cols={5} />
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          onRowClick={(machine: Machine) =>
            router.navigate({
              to: '/machines/$machineId',
              params: { machineId: machine.id },
            })
          }
          emptyMessage="No machines match your filters"
          density="comfortable"
          pageSize={25}
          getRowId={(m) => m.id}
        />
      )}
    </PageContainer>
  );
}
