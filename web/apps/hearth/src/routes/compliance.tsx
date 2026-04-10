import { useState, useMemo } from 'react';
import {
  PageContainer,
  PageHeader,
  DataTable,
  MetricTile,
  Tabs,
  StatusChip,
  SegmentedControl,
  Card,
  ConfirmDialog,
  Tooltip,
  Callout,
  SkeletonTable,
} from '@hearth/ui';
import type { ColumnDef } from '@tanstack/react-table';
import { useComplianceReport } from '../api/reports';
import {
  useDriftedMachines,
  useCompliancePolicies,
  useDeletePolicy,
} from '../api/compliance';
import type { DriftedMachine, DriftStatus, CompliancePolicy } from '../api/types';
import { useRouter } from '@tanstack/react-router';
import { formatRelativeTime, truncateStorePath } from '../lib/time';
import { chartTooltipContent } from '../components/charts/ChartTooltip';
import { LuShieldCheck, LuShieldAlert, LuShieldQuestion, LuTrash2 } from 'react-icons/lu';
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip as RTooltip } from 'recharts';

type DriftFilter = 'all' | 'drifted' | 'compliant' | 'no_target';

const driftColumns: ColumnDef<DriftedMachine, unknown>[] = [
  {
    accessorKey: 'hostname',
    header: 'Hostname',
    cell: ({ getValue }) => (
      <span className="font-semibold text-[var(--color-text-primary)]">{getValue<string>()}</span>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ getValue }) => (
      <span className="text-[var(--color-text-secondary)] capitalize text-sm">
        {getValue<string>() ?? '—'}
      </span>
    ),
  },
  {
    accessorKey: 'drift_status',
    header: 'Status',
    cell: ({ getValue }) => <StatusChip status={getValue<DriftStatus>()} />,
  },
  {
    accessorKey: 'current_closure',
    header: 'Current',
    cell: ({ getValue }) => {
      const v = getValue<string>();
      return (
        <Tooltip content={v ?? 'none'}>
          <span
            className="font-mono text-[var(--color-text-secondary)] text-xs"
           
          >
            {v ? truncateStorePath(v) : '—'}
          </span>
        </Tooltip>
      );
    },
  },
  {
    accessorKey: 'target_closure',
    header: 'Target',
    cell: ({ getValue }) => {
      const v = getValue<string>();
      return (
        <Tooltip content={v ?? 'none'}>
          <span
            className="font-mono text-[var(--color-text-secondary)] text-xs"
           
          >
            {v ? truncateStorePath(v) : '—'}
          </span>
        </Tooltip>
      );
    },
  },
  {
    accessorKey: 'last_heartbeat',
    header: 'Last seen',
    cell: ({ getValue }) => (
      <span className="text-[var(--color-text-tertiary)] text-xs">
        {getValue<string>() ? formatRelativeTime(getValue<string>()!) : 'never'}
      </span>
    ),
  },
];

const CHART_TONES = ['var(--chart-3)', 'var(--chart-1)', 'var(--chart-axis)'];

export function CompliancePage() {
  const [driftFilter, setDriftFilter] = useState<DriftFilter>('all');
  const [activeTab, setActiveTab] = useState('drift');
  const [pendingDelete, setPendingDelete] = useState<CompliancePolicy | null>(null);
  const router = useRouter();

  const { data: compliance } = useComplianceReport();
  const { data: machines, isLoading: machinesLoading } = useDriftedMachines(
    driftFilter === 'all' ? undefined : driftFilter,
  );
  const { data: policies, isLoading: policiesLoading } = useCompliancePolicies();
  const deletePolicy = useDeletePolicy();

  const compliancePercent =
    compliance && compliance.total > 0
      ? Math.round((compliance.compliant / compliance.total) * 100)
      : 0;

  const pieData = useMemo(
    () =>
      compliance
        ? [
            { name: 'Compliant', value: compliance.compliant },
            { name: 'Drifted', value: compliance.drifted },
            { name: 'No target', value: compliance.no_target },
          ].filter((d) => d.value > 0)
        : [],
    [compliance],
  );

  const policyColumns: ColumnDef<CompliancePolicy, unknown>[] = useMemo(
    () => [
      {
        accessorKey: 'name',
        header: 'Policy',
        cell: ({ getValue }) => (
          <span className="font-semibold text-[var(--color-text-primary)]">
            {getValue<string>()}
          </span>
        ),
      },
      {
        accessorKey: 'severity',
        header: 'Severity',
        cell: ({ getValue }) => <StatusChip status={getValue<string>()} />,
      },
      {
        accessorKey: 'control_id',
        header: 'Control',
        cell: ({ getValue }) => (
          <span
            className="font-mono text-[var(--color-text-secondary)] text-xs"
           
          >
            {getValue<string>() ?? '—'}
          </span>
        ),
      },
      {
        accessorKey: 'enabled',
        header: 'Enabled',
        cell: ({ getValue }) =>
          getValue<boolean>() ? (
            <StatusChip status="success" tone="success" label="Enabled" withDot={false} />
          ) : (
            <StatusChip status="idle" tone="neutral" label="Disabled" withDot={false} />
          ),
      },
      {
        accessorKey: 'nix_expression',
        header: 'Expression',
        cell: ({ getValue }) => (
          <Tooltip content={getValue<string>()} side="top">
            <span
              className="font-mono text-[var(--color-text-secondary)] truncate max-w-[320px] inline-block text-xs"
             
            >
              {getValue<string>()}
            </span>
          </Tooltip>
        ),
      },
      {
        id: 'actions',
        header: '',
        enableSorting: false,
        cell: ({ row }) => (
          <button
            type="button"
            className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--color-text-tertiary)] hover:text-[var(--color-error)] hover:bg-[var(--color-error-faint)] cursor-pointer"
            onClick={(e) => {
              e.stopPropagation();
              setPendingDelete(row.original);
            }}
            aria-label="Delete policy"
          >
            <LuTrash2 size={14} />
          </button>
        ),
      },
    ],
    [],
  );

  const tabs = [
    { id: 'drift', label: 'Drift status', count: machines?.length },
    { id: 'policies', label: 'Policies', count: policies?.length },
  ];

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Observability"
        title="Compliance"
        description="Drift detection, compliance policies, and SBOMs across the fleet."
      />

      <div className="grid grid-cols-1 lg:grid-cols-[1fr_240px] gap-[var(--spacing-card-gap)] mb-[var(--spacing-section)]">
        <div
          className="grid gap-[var(--spacing-card-gap)]"
          style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))' }}
        >
          <MetricTile
            label="Compliant"
            value={compliance?.compliant ?? 0}
            sublabel={compliance && compliance.total > 0 ? `${compliancePercent}% of fleet` : undefined}
            icon={<LuShieldCheck size={18} />}
            tone="success"
          />
          <MetricTile
            label="Drifted"
            value={compliance?.drifted ?? 0}
            icon={<LuShieldAlert size={18} />}
            tone={compliance && compliance.drifted > 0 ? 'danger' : 'default'}
          />
          <MetricTile
            label="No target"
            value={compliance?.no_target ?? 0}
            icon={<LuShieldQuestion size={18} />}
            tone="default"
          />
        </div>
        {pieData.length > 0 && (
          <Card>
            <div
              className="uppercase font-semibold text-[var(--color-text-tertiary)] mb-2 text-2xs tracking-wide"
             
            >
              Posture
            </div>
            <ResponsiveContainer width="100%" height={150}>
              <PieChart>
                <Pie
                  data={pieData}
                  dataKey="value"
                  cx="50%"
                  cy="50%"
                  innerRadius={42}
                  outerRadius={62}
                  paddingAngle={2}
                  strokeWidth={0}
                >
                  {pieData.map((_entry, index) => (
                    <Cell key={index} fill={CHART_TONES[index % CHART_TONES.length]} />
                  ))}
                </Pie>
                <RTooltip contentStyle={chartTooltipContent} />
              </PieChart>
            </ResponsiveContainer>
          </Card>
        )}
      </div>

      <Tabs tabs={tabs} activeId={activeTab} onChange={setActiveTab} />

      <div className="mt-6">
        {activeTab === 'drift' && (
          <>
            <div className="mb-4">
              <SegmentedControl
                value={driftFilter}
                onChange={setDriftFilter}
                options={[
                  { value: 'all', label: 'All' },
                  { value: 'drifted', label: 'Drifted' },
                  { value: 'compliant', label: 'Compliant' },
                  { value: 'no_target', label: 'No target' },
                ]}
              />
            </div>
            {machinesLoading ? (
              <SkeletonTable rows={5} cols={6} />
            ) : (
              <DataTable
                data={machines ?? []}
                columns={driftColumns}
                onRowClick={(row) =>
                  router.navigate({
                    to: '/machines/$machineId',
                    params: { machineId: row.id },
                  })
                }
                emptyMessage="No machines match the filter"
                density="comfortable"
                pageSize={25}
              />
            )}
          </>
        )}

        {activeTab === 'policies' && (
          <>
            {policiesLoading ? (
              <SkeletonTable rows={5} cols={6} />
            ) : !policies || policies.length === 0 ? (
              <Callout variant="info" title="No policies defined">
                Compliance policies live in your control plane. Add some via the API to start
                evaluating fleet posture.
              </Callout>
            ) : (
              <DataTable
                data={policies}
                columns={policyColumns}
                density="comfortable"
                pageSize={25}
                emptyMessage="No policies"
              />
            )}
          </>
        )}
      </div>

      <ConfirmDialog
        open={pendingDelete !== null}
        onOpenChange={(o) => !o && setPendingDelete(null)}
        title="Delete policy"
        description={
          pendingDelete
            ? `Are you sure you want to delete "${pendingDelete.name}"? This cannot be undone.`
            : ''
        }
        confirmLabel="Delete policy"
        variant="danger"
        onConfirm={() => {
          if (pendingDelete) deletePolicy.mutate(pendingDelete.id);
          setPendingDelete(null);
        }}
      />
    </PageContainer>
  );
}
