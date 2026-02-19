import { useState, useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, SearchInput } from '@hearth/ui';
import { useMachines } from '../../api/machines';
import type { Machine, EnrollmentStatus } from '../../api/types';
import { LuMonitor } from 'react-icons/lu';
import { formatRelativeTime } from '../../lib/time';

const enrollmentColors: Record<EnrollmentStatus, string> = {
  active: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  enrolled: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  approved: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  provisioning: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  decommissioned: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const columns: ColumnDef<Machine, unknown>[] = [
  {
    accessorKey: 'hostname',
    header: 'Hostname',
    cell: ({ row }) => (
      <div className="flex items-center gap-2">
        <LuMonitor size={14} className="text-[var(--color-text-tertiary)] shrink-0" />
        <span className="font-medium">{row.original.hostname}</span>
      </div>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.role ?? '—'}
      </span>
    ),
  },
  {
    accessorKey: 'enrollment_status',
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.enrollment_status;
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${enrollmentColors[status]}`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              status === 'active' || status === 'enrolled'
                ? 'bg-[var(--color-success)]'
                : status === 'pending'
                  ? 'bg-[var(--color-warning)] animate-[pulse-dot_1.8s_ease-in-out_infinite]'
                  : status === 'approved' || status === 'provisioning'
                    ? 'bg-[var(--color-info)] animate-[pulse-dot_1.8s_ease-in-out_infinite]'
                    : 'bg-[var(--color-error)]'
            }`}
          />
          {status}
        </span>
      );
    },
  },
  {
    accessorKey: 'last_heartbeat',
    header: 'Last Heartbeat',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.last_heartbeat
          ? formatRelativeTime(row.original.last_heartbeat)
          : 'Never'}
      </span>
    ),
  },
  {
    accessorKey: 'tags',
    header: 'Tags',
    enableSorting: false,
    cell: ({ row }) => (
      <div className="flex flex-wrap gap-1">
        {row.original.tags.length === 0 ? (
          <span className="text-sm text-[var(--color-text-tertiary)]">—</span>
        ) : (
          row.original.tags.map((tag) => (
            <span
              key={tag}
              className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)]"
            >
              {tag}
            </span>
          ))
        )}
      </div>
    ),
  },
];

export function MachinesPage() {
  const router = useRouter();
  const { data: machines, isLoading } = useMachines();
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!machines) return [];
    if (!search) return machines;
    const q = search.toLowerCase();
    return machines.filter(
      (m) =>
        m.hostname.toLowerCase().includes(q) ||
        (m.role ?? '').toLowerCase().includes(q) ||
        m.enrollment_status.toLowerCase().includes(q) ||
        m.tags.some((t) => t.toLowerCase().includes(q)),
    );
  }, [machines, search]);

  return (
    <div>
      <PageHeader
        title="Machines"
        description="Manage and monitor all fleet devices"
      />

      <div className="mb-4 max-w-sm">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by hostname, role, status, or tag..."
        />
      </div>

      {isLoading ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading machines...
        </p>
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          onRowClick={(machine) =>
            router.navigate({
              to: '/machines/$machineId',
              params: { machineId: machine.id },
            })
          }
          emptyMessage="No machines found"
        />
      )}
    </div>
  );
}
