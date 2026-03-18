import { useState, useMemo } from 'react';
import { PageHeader, FilterPills, DataTable, StatCard } from '@hearth/ui';
import type { ColumnDef } from '@tanstack/react-table';
import { useComplianceReport } from '../api/reports';
import { useDriftedMachines, useCompliancePolicies, useDeletePolicy } from '../api/compliance';
import type { DriftedMachine, DriftStatus, CompliancePolicy } from '../api/types';
import { useRouter } from '@tanstack/react-router';
import { formatRelativeTime, truncateStorePath } from '../lib/time';
import {
  LuShield,
  LuShieldCheck,
  LuShieldAlert,
  LuShieldQuestion,
  LuTrash2,
} from 'react-icons/lu';
import {
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
  Tooltip,
} from 'recharts';

type FilterValue = 'All' | 'Drifted' | 'Compliant' | 'No Target';

const filterToStatus: Record<FilterValue, DriftStatus | 'all'> = {
  All: 'all',
  Drifted: 'drifted',
  Compliant: 'compliant',
  'No Target': 'no_target',
};

const driftColumns: ColumnDef<DriftedMachine, unknown>[] = [
  {
    accessorKey: 'hostname',
    header: 'Hostname',
    cell: ({ getValue }) => (
      <span className="font-mono text-xs">{getValue<string>()}</span>
    ),
  },
  {
    accessorKey: 'role',
    header: 'Role',
    cell: ({ getValue }) => getValue<string>() ?? '—',
  },
  {
    accessorKey: 'drift_status',
    header: 'Status',
    cell: ({ getValue }) => {
      const status = getValue<DriftStatus>();
      const colors: Record<DriftStatus, string> = {
        compliant: 'text-[var(--color-success)]',
        drifted: 'text-[var(--color-error)]',
        no_target: 'text-[var(--color-text-tertiary)]',
      };
      const labels: Record<DriftStatus, string> = {
        compliant: 'Compliant',
        drifted: 'Drifted',
        no_target: 'No Target',
      };
      return <span className={`text-xs font-medium ${colors[status]}`}>{labels[status]}</span>;
    },
  },
  {
    accessorKey: 'current_closure',
    header: 'Current',
    cell: ({ getValue }) => {
      const v = getValue<string>();
      return (
        <span className="font-mono text-xs text-[var(--color-text-secondary)]" title={v ?? ''}>
          {v ? truncateStorePath(v) : '—'}
        </span>
      );
    },
  },
  {
    accessorKey: 'target_closure',
    header: 'Target',
    cell: ({ getValue }) => {
      const v = getValue<string>();
      return (
        <span className="font-mono text-xs text-[var(--color-text-secondary)]" title={v ?? ''}>
          {v ? truncateStorePath(v) : '—'}
        </span>
      );
    },
  },
  {
    accessorKey: 'last_heartbeat',
    header: 'Last Seen',
    cell: ({ getValue }) => {
      const v = getValue<string>();
      return (
        <span className="text-xs text-[var(--color-text-tertiary)]">
          {v ? formatRelativeTime(v) : 'never'}
        </span>
      );
    },
  },
];

const severityColors: Record<string, string> = {
  critical: 'text-[var(--color-error)]',
  high: 'text-[var(--color-warning)]',
  medium: 'text-[var(--color-text-primary)]',
  low: 'text-[var(--color-text-tertiary)]',
};

const CHART_COLORS = [
  'var(--color-success)',
  'var(--color-error)',
  'var(--color-text-tertiary)',
];

export function CompliancePage() {
  const [filter, setFilter] = useState<FilterValue>('All');
  const [activeTab, setActiveTab] = useState<'drift' | 'policies'>('drift');
  const router = useRouter();

  const { data: compliance } = useComplianceReport();
  const { data: machines, isLoading: machinesLoading } = useDriftedMachines(filterToStatus[filter]);
  const { data: policies, isLoading: policiesLoading } = useCompliancePolicies();
  const deletePolicy = useDeletePolicy();

  const compliancePercent = compliance && compliance.total > 0
    ? Math.round((compliance.compliant / compliance.total) * 100)
    : 0;

  const pieData = compliance
    ? [
        { name: 'Compliant', value: compliance.compliant },
        { name: 'Drifted', value: compliance.drifted },
        { name: 'No Target', value: compliance.no_target },
      ].filter((d) => d.value > 0)
    : [];

  const policyColumns: ColumnDef<CompliancePolicy, unknown>[] = useMemo(() => [
    {
      accessorKey: 'name',
      header: 'Policy Name',
      cell: ({ getValue }) => (
        <span className="font-medium text-sm">{getValue<string>()}</span>
      ),
    },
    {
      accessorKey: 'severity',
      header: 'Severity',
      cell: ({ getValue }) => {
        const sev = getValue<string>();
        return (
          <span className={`text-xs font-medium uppercase ${severityColors[sev] ?? ''}`}>
            {sev}
          </span>
        );
      },
    },
    {
      accessorKey: 'control_id',
      header: 'Control ID',
      cell: ({ getValue }) => (
        <span className="font-mono text-xs">{getValue<string>() ?? '—'}</span>
      ),
    },
    {
      accessorKey: 'enabled',
      header: 'Enabled',
      cell: ({ getValue }) => (
        <span className={`text-xs ${getValue<boolean>() ? 'text-[var(--color-success)]' : 'text-[var(--color-text-tertiary)]'}`}>
          {getValue<boolean>() ? 'Yes' : 'No'}
        </span>
      ),
    },
    {
      accessorKey: 'nix_expression',
      header: 'Expression',
      cell: ({ getValue }) => (
        <span className="font-mono text-xs text-[var(--color-text-secondary)] truncate max-w-[300px] inline-block">
          {getValue<string>()}
        </span>
      ),
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <button
          type="button"
          className="text-[var(--color-text-tertiary)] hover:text-[var(--color-error)] cursor-pointer"
          onClick={(e) => {
            e.stopPropagation();
            if (confirm(`Delete policy "${row.original.name}"?`)) {
              deletePolicy.mutate(row.original.id);
            }
          }}
        >
          <LuTrash2 size={14} />
        </button>
      ),
    },
  // eslint-disable-next-line react-hooks/exhaustive-deps
  ], [deletePolicy.mutate]);

  return (
    <div>
      <PageHeader title="Compliance" description="Fleet drift status, compliance policies, and SBOMs" />

      {/* Summary Cards + Chart */}
      <section className="mb-8">
        <div className="grid grid-cols-1 lg:grid-cols-[1fr_200px] gap-6">
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <StatCard
              icon={<LuShieldCheck size={20} />}
              value={compliance?.compliant ?? 0}
              label="Compliant"
              trend={compliance && compliance.total > 0
                ? { value: `${compliancePercent}%`, positive: true }
                : undefined
              }
            />
            <StatCard
              icon={<LuShieldAlert size={20} />}
              value={compliance?.drifted ?? 0}
              label="Drifted"
              trend={compliance && compliance.drifted > 0
                ? { value: `${compliance.drifted}`, positive: false }
                : undefined
              }
            />
            <StatCard
              icon={<LuShieldQuestion size={20} />}
              value={compliance?.no_target ?? 0}
              label="No Target"
            />
          </div>
          {pieData.length > 0 && (
            <div className="flex items-center justify-center">
              <ResponsiveContainer width={160} height={160}>
                <PieChart>
                  <Pie
                    data={pieData}
                    dataKey="value"
                    cx="50%"
                    cy="50%"
                    innerRadius={45}
                    outerRadius={70}
                    paddingAngle={2}
                    strokeWidth={0}
                  >
                    {pieData.map((_entry, index) => (
                      <Cell key={index} fill={CHART_COLORS[index % CHART_COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      backgroundColor: 'var(--color-surface)',
                      border: '1px solid var(--color-border-subtle)',
                      borderRadius: 'var(--radius-sm)',
                      fontSize: 12,
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
          )}
        </div>
      </section>

      {/* Tab Switch */}
      <div className="flex gap-4 mb-6 border-b border-[var(--color-border-subtle)]">
        <button
          type="button"
          className={`pb-2 text-sm font-medium cursor-pointer transition-colors ${
            activeTab === 'drift'
              ? 'text-[var(--color-ember)] border-b-2 border-[var(--color-ember)]'
              : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
          }`}
          onClick={() => setActiveTab('drift')}
        >
          <LuShield size={14} className="inline mr-1.5" />
          Drift Status
        </button>
        <button
          type="button"
          className={`pb-2 text-sm font-medium cursor-pointer transition-colors ${
            activeTab === 'policies'
              ? 'text-[var(--color-ember)] border-b-2 border-[var(--color-ember)]'
              : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
          }`}
          onClick={() => setActiveTab('policies')}
        >
          <LuShieldCheck size={14} className="inline mr-1.5" />
          Policies ({policies?.length ?? 0})
        </button>
      </div>

      {/* Drift Tab */}
      {activeTab === 'drift' && (
        <section>
          <div className="mb-4">
            <FilterPills
              options={['Drifted', 'Compliant', 'No Target']}
              active={filter}
              onSelect={(v) => setFilter(v as FilterValue)}
            />
          </div>

          {machinesLoading ? (
            <p className="text-sm text-[var(--color-text-tertiary)]">Loading drift data...</p>
          ) : (
            <DataTable
              data={machines ?? []}
              columns={driftColumns}
              onRowClick={(row) => router.navigate({ to: `/machines/${row.id}` })}
              emptyMessage="No machines match the selected filter."
            />
          )}
        </section>
      )}

      {/* Policies Tab */}
      {activeTab === 'policies' && (
        <section>
          {policiesLoading ? (
            <p className="text-sm text-[var(--color-text-tertiary)]">Loading policies...</p>
          ) : (
            <DataTable
              data={policies ?? []}
              columns={policyColumns}
              emptyMessage="No compliance policies defined yet."
            />
          )}
        </section>
      )}
    </div>
  );
}
