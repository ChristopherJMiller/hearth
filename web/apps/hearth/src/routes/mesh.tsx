import { useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  Callout,
  SkeletonTable,
  StatusChip,
  MetricTile,
} from '@hearth/ui';
import { useMachines } from '../api/machines';
import type { Machine } from '../api/types';
import { formatRelativeTime } from '../lib/time';
import { LuNetwork, LuMonitor, LuRadio } from 'react-icons/lu';

const meshColumns: ColumnDef<Machine, unknown>[] = [
  {
    accessorKey: 'hostname',
    header: 'Hostname',
    cell: ({ row }) => (
      <div className="flex items-center gap-2">
        <LuMonitor size={14} className="text-text-tertiary" />
        <span className="font-semibold text-text-primary">{row.original.hostname}</span>
      </div>
    ),
  },
  {
    accessorKey: 'headscale_ip',
    header: 'Mesh IP',
    cell: ({ row }) =>
      row.original.headscale_ip ? (
        <span className="font-mono text-text-secondary text-xs">
          {row.original.headscale_ip}
        </span>
      ) : (
        <span className="italic text-text-tertiary text-xs">
          not connected
        </span>
      ),
  },
  {
    accessorKey: 'headscale_node_id',
    header: 'Node ID',
    cell: ({ row }) =>
      row.original.headscale_node_id ? (
        <span className="font-mono text-text-secondary text-xs">
          {row.original.headscale_node_id}
        </span>
      ) : (
        <span className="text-text-tertiary text-xs">—</span>
      ),
  },
  {
    accessorKey: 'enrollment_status',
    header: 'Status',
    cell: ({ row }) => <StatusChip status={row.original.enrollment_status} />,
  },
  {
    accessorKey: 'last_heartbeat',
    header: 'Last heartbeat',
    cell: ({ row }) => (
      <span className="text-text-secondary text-xs">
        {row.original.last_heartbeat ? formatRelativeTime(row.original.last_heartbeat) : 'never'}
      </span>
    ),
  },
];

export function MeshPage() {
  const { data: machines, isLoading, isError } = useMachines();

  const counts = useMemo(() => {
    const list = machines ?? [];
    const connected = list.filter((m) => !!m.headscale_ip).length;
    return { total: list.length, connected, offline: list.length - connected };
  }, [machines]);

  const sorted = useMemo(() => {
    return [...(machines ?? [])].sort((a, b) => {
      // Connected first, then by IP
      if (!!a.headscale_ip !== !!b.headscale_ip) return a.headscale_ip ? -1 : 1;
      return (a.headscale_ip ?? '').localeCompare(b.headscale_ip ?? '');
    });
  }, [machines]);

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Fleet"
        title="Mesh"
        description="Headscale-managed WireGuard mesh connecting your fleet. Live topology coming soon."
      />

      <div
        className="grid gap-card-gap"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
      >
        <MetricTile label="Mesh nodes" value={counts.total} icon={<LuNetwork size={18} />} tone="ember" />
        <MetricTile label="Connected" value={counts.connected} icon={<LuRadio size={18} />} tone="success" />
        <MetricTile
          label="Offline"
          value={counts.offline}
          icon={<LuMonitor size={18} />}
          tone={counts.offline > 0 ? 'warning' : 'default'}
        />
      </div>

      <div className="mb-6">
        <Callout variant="info" title="Live topology coming soon">
          A force-directed view of mesh peers, latencies, and route health will live here once the
          control plane exposes Headscale topology data.
        </Callout>
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load mesh state" />
      ) : isLoading ? (
        <SkeletonTable rows={6} cols={5} />
      ) : (
        <DataTable
          data={sorted}
          columns={meshColumns}
          density="comfortable"
          pageSize={50}
          emptyMessage="No machines have joined the mesh"
        />
      )}
    </PageContainer>
  );
}
