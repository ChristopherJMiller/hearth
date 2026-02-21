import React, { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, FilterPills, Button } from '@hearth/ui';
import {
  useSoftwareRequests,
  useApproveRequest,
  useDenyRequest,
} from '../../api/requests';
import type { SoftwareRequest } from '../../api/types';
import { formatRelativeTime, truncateId } from '../../lib/time';
import { LuCheck, LuX } from 'react-icons/lu';

type RequestStatus = SoftwareRequest['status'];

const statusColors: Record<RequestStatus, string> = {
  pending: 'bg-[var(--color-warning-faint)] text-[var(--color-warning)]',
  approved: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  denied: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
  installing: 'bg-[var(--color-info-faint)] text-[var(--color-info)]',
  installed: 'bg-[var(--color-success-faint)] text-[var(--color-success)]',
  failed: 'bg-[var(--color-error-faint)] text-[var(--color-error)]',
};

const filterOptions = ['Pending', 'Approved', 'Denied', 'Installing', 'Installed', 'Failed'];

function ActionsCell({ request }: { request: SoftwareRequest }) {
  const approve = useApproveRequest();
  const deny = useDenyRequest();

  if (request.status !== 'pending') {
    return <span className="text-xs text-[var(--color-text-tertiary)]">—</span>;
  }

  return (
    <div className="flex items-center gap-2">
      <Button
        variant="primary"
        size="sm"
        onClick={(e: React.MouseEvent<HTMLButtonElement>) => {
          e.stopPropagation();
          approve.mutate({ id: request.id, admin: 'console-admin' });
        }}
        disabled={approve.isPending || deny.isPending}
      >
        <LuCheck size={14} />
        Approve
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="text-[var(--color-error)] hover:text-[var(--color-error)] hover:bg-[var(--color-error-faint)]"
        onClick={(e: React.MouseEvent<HTMLButtonElement>) => {
          e.stopPropagation();
          deny.mutate({ id: request.id, admin: 'console-admin' });
        }}
        disabled={approve.isPending || deny.isPending}
      >
        <LuX size={14} />
        Deny
      </Button>
    </div>
  );
}

const columns: ColumnDef<SoftwareRequest, unknown>[] = [
  {
    accessorKey: 'id',
    header: 'ID',
    cell: ({ row }) => (
      <span className="font-mono text-xs" title={row.original.id}>
        {truncateId(row.original.id)}
      </span>
    ),
  },
  {
    accessorKey: 'username',
    header: 'Username',
    cell: ({ row }) => (
      <span className="font-medium">{row.original.username}</span>
    ),
  },
  {
    accessorKey: 'machine_id',
    header: 'Machine',
    cell: ({ row }) => (
      <span className="font-mono text-xs" title={row.original.machine_id}>
        {truncateId(row.original.machine_id)}
      </span>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => {
      const status = row.original.status;
      const isPulsing = status === 'pending' || status === 'installing';
      return (
        <span
          className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-1 rounded-full whitespace-nowrap ${statusColors[status]}`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              status === 'installed'
                ? 'bg-[var(--color-success)]'
                : status === 'denied' || status === 'failed'
                  ? 'bg-[var(--color-error)]'
                  : status === 'pending'
                    ? 'bg-[var(--color-warning)]'
                    : 'bg-[var(--color-info)]'
            } ${isPulsing ? 'animate-[pulse-dot_1.8s_ease-in-out_infinite]' : ''}`}
          />
          {status}
        </span>
      );
    },
  },
  {
    accessorKey: 'requested_at',
    header: 'Requested',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
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
  const { data: requests, isLoading } = useSoftwareRequests();
  const [activeFilter, setActiveFilter] = useState('All');

  const filtered = useMemo(() => {
    if (!requests) return [];
    if (activeFilter === 'All') return requests;
    const filterValue = activeFilter.toLowerCase();
    return requests.filter((r) => r.status === filterValue);
  }, [requests, activeFilter]);

  return (
    <div>
      <PageHeader
        title="Software Requests"
        description="Review and manage user software installation requests"
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
          Loading requests...
        </p>
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No software requests found"
        />
      )}
    </div>
  );
}
