import { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, SearchInput } from '@hearth/ui';
import { useAuditLog } from '../../api/audit';
import type { AuditEvent } from '../../api/types';
import { formatRelativeTime, truncateId, truncate } from '../../lib/time';
import { LuFileText, LuUser, LuMonitor, LuClock } from 'react-icons/lu';

const eventTypeColors: Record<string, string> = {
  enrollment: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  deployment: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  heartbeat: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  request: 'bg-[var(--color-purple-faint)] text-[var(--color-purple)]',
  config: 'bg-[var(--color-ember-faint)] text-[var(--color-ember)]',
};

function getEventTypeColor(eventType: string): string {
  const key = Object.keys(eventTypeColors).find((k) =>
    eventType.toLowerCase().includes(k),
  );
  return key
    ? eventTypeColors[key]
    : 'bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)]';
}

const columns: ColumnDef<AuditEvent, unknown>[] = [
  {
    accessorKey: 'event_type',
    header: 'Event Type',
    cell: ({ row }) => {
      const eventType = row.original.event_type;
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${getEventTypeColor(eventType)}`}
        >
          <LuFileText size={12} />
          {eventType}
        </span>
      );
    },
  },
  {
    accessorKey: 'actor',
    header: 'Actor',
    cell: ({ row }) => (
      <div className="flex items-center gap-1.5">
        <LuUser size={13} className="text-[var(--color-text-tertiary)] shrink-0" />
        <span className="text-sm text-[var(--color-text-secondary)]">
          {row.original.actor ?? '—'}
        </span>
      </div>
    ),
  },
  {
    accessorKey: 'machine_id',
    header: 'Machine ID',
    cell: ({ row }) => {
      const mid = row.original.machine_id;
      if (!mid) return <span className="text-sm text-[var(--color-text-tertiary)]">—</span>;
      return (
        <div className="flex items-center gap-1.5">
          <LuMonitor size={13} className="text-[var(--color-text-tertiary)] shrink-0" />
          <span className="font-mono text-xs" title={mid}>
            {truncateId(mid)}
          </span>
        </div>
      );
    },
  },
  {
    accessorKey: 'created_at',
    header: 'Timestamp',
    cell: ({ row }) => (
      <div className="flex items-center gap-1.5">
        <LuClock size={13} className="text-[var(--color-text-tertiary)] shrink-0" />
        <span className="text-sm text-[var(--color-text-secondary)]">
          {formatRelativeTime(row.original.created_at)}
        </span>
      </div>
    ),
  },
  {
    accessorKey: 'details',
    header: 'Details',
    enableSorting: false,
    cell: ({ row }) => {
      const details = row.original.details;
      const json = JSON.stringify(details);
      if (json === '{}' || json === 'null') {
        return <span className="text-sm text-[var(--color-text-tertiary)]">—</span>;
      }
      return (
        <span
          className="text-xs font-mono text-[var(--color-text-secondary)] max-w-[250px] inline-block truncate"
          title={JSON.stringify(details, null, 2)}
        >
          {truncate(json, 60)}
        </span>
      );
    },
  },
];

export function AuditPage() {
  const { data: events, isLoading } = useAuditLog();
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!events) return [];
    if (!search) return events;
    const q = search.toLowerCase();
    return events.filter(
      (e) =>
        e.event_type.toLowerCase().includes(q) ||
        (e.actor ?? '').toLowerCase().includes(q) ||
        (e.machine_id ?? '').toLowerCase().includes(q) ||
        JSON.stringify(e.details).toLowerCase().includes(q),
    );
  }, [events, search]);

  return (
    <div>
      <PageHeader
        title="Audit Log"
        description="View all system events and actions"
      />

      <div className="mb-4 max-w-sm">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by event type, actor, or machine..."
        />
      </div>

      {isLoading ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading audit events...
        </p>
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No audit events found"
        />
      )}
    </div>
  );
}
