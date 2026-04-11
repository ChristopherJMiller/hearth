import { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  SearchInput,
  StatusChip,
  SegmentedControl,
  Tooltip,
  Callout,
  SkeletonTable,
} from '@hearth/ui';
import { useAuditLog } from '../../api/audit';
import type { AuditEvent } from '../../api/types';
import { formatRelativeTime, truncateId } from '../../lib/time';
import { LuUser, LuMonitor, LuClock } from 'react-icons/lu';

type ViewMode = 'table' | 'compact';

const columns: ColumnDef<AuditEvent, unknown>[] = [
  {
    accessorKey: 'event_type',
    header: 'Event',
    cell: ({ row }) => {
      const eventType = row.original.event_type;
      let tone: 'info' | 'success' | 'warning' | 'purple' | 'neutral' = 'neutral';
      if (eventType.includes('enrollment')) tone = 'warning';
      else if (eventType.includes('deployment')) tone = 'info';
      else if (eventType.includes('heartbeat')) tone = 'success';
      else if (eventType.includes('request') || eventType.includes('install')) tone = 'purple';
      return <StatusChip status="info" tone={tone} label={eventType} withDot={false} />;
    },
  },
  {
    accessorKey: 'actor',
    header: 'Actor',
    cell: ({ row }) => (
      <div className="flex items-center gap-2">
        <LuUser size={13} className="text-text-tertiary shrink-0" />
        <span className="text-text-secondary text-sm">
          {row.original.actor ?? <span className="italic text-text-tertiary">system</span>}
        </span>
      </div>
    ),
  },
  {
    accessorKey: 'machine_id',
    header: 'Machine',
    cell: ({ row }) => {
      const mid = row.original.machine_id;
      if (!mid) return <span className="text-text-tertiary text-xs">—</span>;
      return (
        <Tooltip content={mid}>
          <div className="flex items-center gap-2">
            <LuMonitor size={13} className="text-text-tertiary shrink-0" />
            <span
              className="font-mono text-text-secondary text-xs"
             
            >
              {truncateId(mid)}
            </span>
          </div>
        </Tooltip>
      );
    },
  },
  {
    accessorKey: 'created_at',
    header: 'When',
    cell: ({ row }) => (
      <div className="flex items-center gap-2">
        <LuClock size={13} className="text-text-tertiary shrink-0" />
        <span className="text-text-secondary text-xs">
          {formatRelativeTime(row.original.created_at)}
        </span>
      </div>
    ),
  },
];

export function AuditPage() {
  const { data: events, isLoading, isError } = useAuditLog({ limit: 500 });
  const [search, setSearch] = useState('');
  const [view, setView] = useState<ViewMode>('table');
  const [eventTypeFilter, setEventTypeFilter] = useState<string>('all');

  const filtered = useMemo(() => {
    if (!events) return [];
    let result = events;
    if (eventTypeFilter !== 'all') {
      result = result.filter((e) => e.event_type.toLowerCase().includes(eventTypeFilter));
    }
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(
        (e) =>
          e.event_type.toLowerCase().includes(q) ||
          (e.actor ?? '').toLowerCase().includes(q) ||
          (e.machine_id ?? '').toLowerCase().includes(q) ||
          JSON.stringify(e.details).toLowerCase().includes(q),
      );
    }
    return result;
  }, [events, search, eventTypeFilter]);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Identity & access"
        title="Audit log"
        description="An immutable trail of every consequential action across the fleet — searchable and filterable."
      />

      <div className="flex items-center justify-between gap-3 flex-wrap mb-4">
        <div className="flex items-center gap-3 flex-wrap">
          <SegmentedControl
            value={eventTypeFilter}
            onChange={setEventTypeFilter}
            options={[
              { value: 'all', label: 'All' },
              { value: 'enrollment', label: 'Enrollment' },
              { value: 'deployment', label: 'Deployment' },
              { value: 'request', label: 'Requests' },
              { value: 'heartbeat', label: 'Heartbeat' },
            ]}
          />
          <SegmentedControl
            value={view}
            onChange={setView}
            size="sm"
            options={[
              { value: 'table', label: 'Table' },
              { value: 'compact', label: 'Compact' },
            ]}
          />
        </div>
        <div className="min-w-[280px] flex-1 max-w-md">
          <SearchInput
            value={search}
            onChange={setSearch}
            placeholder="Search by event, actor, machine, or detail…"
          />
        </div>
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load audit events" />
      ) : isLoading ? (
        <SkeletonTable rows={10} cols={4} />
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No audit events match your filter"
          density={view === 'compact' ? 'cozy' : 'comfortable'}
          pageSize={50}
          renderExpanded={(row) => (
            <pre
              className="font-mono whitespace-pre-wrap text-text-secondary text-2xs"
             
            >
              {JSON.stringify(row.details, null, 2)}
            </pre>
          )}
        />
      )}
    </PageContainer>
  );
}
