import React, { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  SegmentedControl,
  Button,
  StatusChip,
  Tooltip,
  Callout,
  SkeletonTable,
  MetricTile,
} from '@hearth/ui';
import {
  useSoftwareRequests,
  useApproveRequest,
  useDenyRequest,
} from '../../api/requests';
import type { SoftwareRequest } from '../../api/types';
import { useActor } from '../../hooks/useActor';
import { formatRelativeTime, truncateId } from '../../lib/time';
import { LuCheck, LuX, LuInbox, LuClock, LuCheckCircle, LuXCircle } from 'react-icons/lu';

type Filter = 'all' | 'pending' | 'approved' | 'denied' | 'installed' | 'failed';

function ActionsCell({ request }: { request: SoftwareRequest }) {
  const approve = useApproveRequest();
  const deny = useDenyRequest();
  const actor = useActor();

  if (request.status !== 'pending') {
    return <span className="text-text-tertiary text-xs">—</span>;
  }

  const busy = approve.isPending || deny.isPending;

  return (
    <div className="flex items-center gap-1.5">
      <Button
        variant="primary"
        size="sm"
        loading={approve.isPending}
        leadingIcon={<LuCheck size={13} />}
        onClick={(e: React.MouseEvent<HTMLButtonElement>) => {
          e.stopPropagation();
          approve.mutate({ id: request.id, admin: actor });
        }}
        disabled={busy}
      >
        Approve
      </Button>
      <Button
        variant="ghost"
        size="sm"
        leadingIcon={<LuX size={13} />}
        onClick={(e: React.MouseEvent<HTMLButtonElement>) => {
          e.stopPropagation();
          deny.mutate({ id: request.id, admin: actor });
        }}
        disabled={busy}
      >
        Deny
      </Button>
    </div>
  );
}

const columns: ColumnDef<SoftwareRequest, unknown>[] = [
  {
    accessorKey: 'username',
    header: 'User',
    cell: ({ row }) => (
      <span className="font-semibold text-text-primary">{row.original.username}</span>
    ),
  },
  {
    accessorKey: 'machine_id',
    header: 'Machine',
    cell: ({ row }) => (
      <Tooltip content={row.original.machine_id}>
        <span
          className="font-mono text-text-secondary text-xs"
         
        >
          {truncateId(row.original.machine_id)}
        </span>
      </Tooltip>
    ),
  },
  {
    accessorKey: 'catalog_entry_id',
    header: 'Software',
    cell: ({ row }) => (
      <Tooltip content={row.original.catalog_entry_id}>
        <span
          className="font-mono text-text-secondary text-xs"
         
        >
          {truncateId(row.original.catalog_entry_id)}
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
    accessorKey: 'requested_at',
    header: 'Requested',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {formatRelativeTime(row.original.requested_at)}
      </span>
    ),
  },
  {
    id: 'actions',
    header: 'Actions',
    enableSorting: false,
    cell: ({ row }) => <ActionsCell request={row.original} />,
  },
];

export function RequestsPage() {
  const { data: requests, isLoading, isError } = useSoftwareRequests();
  const [filter, setFilter] = useState<Filter>('all');

  const counts = useMemo(() => {
    const list = requests ?? [];
    return {
      total: list.length,
      pending: list.filter((r) => r.status === 'pending').length,
      installed: list.filter((r) => r.status === 'installed').length,
      failed: list.filter((r) => r.status === 'failed' || r.status === 'denied').length,
    };
  }, [requests]);

  const filtered = useMemo(() => {
    const list = requests ?? [];
    if (filter === 'all') return list;
    return list.filter((r) => r.status === filter);
  }, [requests, filter]);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Software"
        title="Requests"
        description="User-initiated installs from the catalog. Approve, deny, and watch them flow through to the agent."
      />

      <div
        className="grid gap-card-gap"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile label="Total" value={counts.total} icon={<LuInbox size={18} />} tone="ember" />
        <MetricTile
          label="Pending"
          value={counts.pending}
          icon={<LuClock size={18} />}
          tone={counts.pending > 0 ? 'warning' : 'default'}
        />
        <MetricTile label="Installed" value={counts.installed} icon={<LuCheckCircle size={18} />} tone="success" />
        <MetricTile
          label="Failed / denied"
          value={counts.failed}
          icon={<LuXCircle size={18} />}
          tone={counts.failed > 0 ? 'danger' : 'default'}
        />
      </div>

      <div className="mb-4">
        <SegmentedControl
          value={filter}
          onChange={setFilter}
          options={[
            { value: 'all', label: `All · ${counts.total}` },
            { value: 'pending', label: `Pending · ${counts.pending}` },
            { value: 'approved', label: 'Approved' },
            { value: 'installed', label: 'Installed' },
            { value: 'denied', label: 'Denied' },
            { value: 'failed', label: 'Failed' },
          ]}
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load requests" />
      ) : isLoading ? (
        <SkeletonTable rows={6} cols={6} />
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No software requests match your filter"
          density="comfortable"
          pageSize={25}
        />
      )}
    </PageContainer>
  );
}
