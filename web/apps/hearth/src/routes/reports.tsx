import { PageHeader, StatCard } from '@hearth/ui';
import { useComplianceReport, useDeploymentTimeline, useEnrollmentTimeline } from '../api/reports';
import {
  BarChart,
  Bar,
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from 'recharts';
import {
  LuMonitor,
  LuCheckCircle,
  LuAlertTriangle,
  LuHelpCircle,
} from 'react-icons/lu';

export function ReportsPage() {
  const { data: compliance, isLoading: complianceLoading, error: complianceError } = useComplianceReport();
  const { data: deploymentTimeline, isLoading: deployLoading, error: deployError } = useDeploymentTimeline();
  const { data: enrollmentTimeline, isLoading: enrollLoading, error: enrollError } = useEnrollmentTimeline();

  return (
    <div>
      <PageHeader title="Reports" description="Fleet compliance and activity metrics" />

      {/* Compliance Section */}
      <section className="mb-8">
        <h2 className="text-sm font-semibold text-[var(--color-text-primary)] mb-4">Compliance Overview</h2>
        {complianceError ? (
          <p className="text-sm text-[var(--color-error)]">Failed to load compliance data.</p>
        ) : complianceLoading ? (
          <p className="text-sm text-[var(--color-text-tertiary)]">Loading compliance data...</p>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            <StatCard
              icon={<LuMonitor size={20} />}
              value={compliance?.total ?? 0}
              label="Total Machines"
            />
            <StatCard
              icon={<LuCheckCircle size={20} />}
              value={compliance?.compliant ?? 0}
              label="Compliant"
              trend={
                compliance && compliance.total > 0
                  ? {
                      value: `${Math.round((compliance.compliant / compliance.total) * 100)}%`,
                      positive: true,
                    }
                  : undefined
              }
            />
            <StatCard
              icon={<LuAlertTriangle size={20} />}
              value={compliance?.drifted ?? 0}
              label="Drifted"
              trend={
                compliance && compliance.drifted > 0
                  ? { value: `${compliance.drifted}`, positive: false }
                  : undefined
              }
            />
            <StatCard
              icon={<LuHelpCircle size={20} />}
              value={compliance?.no_target ?? 0}
              label="No Target Set"
            />
          </div>
        )}
      </section>

      {/* Deployment Timeline */}
      <section className="mb-8">
        <div className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] shadow-[var(--shadow-card)] p-5">
          <h2 className="text-sm font-semibold text-[var(--color-text-primary)] mb-4">Deployment Activity (30 days)</h2>
          {deployError ? (
            <p className="text-sm text-[var(--color-error)]">Failed to load deployment timeline.</p>
          ) : deployLoading ? (
            <p className="text-sm text-[var(--color-text-tertiary)]">Loading deployment timeline...</p>
          ) : !deploymentTimeline || deploymentTimeline.length === 0 ? (
            <p className="text-sm text-[var(--color-text-tertiary)] py-8 text-center">No deployment data available.</p>
          ) : (
            <ResponsiveContainer width="100%" height={300}>
              <BarChart data={deploymentTimeline} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-subtle)" />
                <XAxis
                  dataKey="date"
                  tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                  tickFormatter={(v: string) => {
                    const d = new Date(v);
                    return `${d.getMonth() + 1}/${d.getDate()}`;
                  }}
                />
                <YAxis
                  tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                  allowDecimals={false}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: 'var(--color-surface)',
                    border: '1px solid var(--color-border-subtle)',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: 12,
                  }}
                />
                <Legend wrapperStyle={{ fontSize: 12 }} />
                <Bar dataKey="completed" name="Completed" fill="var(--color-success)" radius={[2, 2, 0, 0]} />
                <Bar dataKey="failed" name="Failed" fill="var(--color-error)" radius={[2, 2, 0, 0]} />
                <Bar dataKey="rolled_back" name="Rolled Back" fill="var(--color-warning)" radius={[2, 2, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>
      </section>

      {/* Enrollment Timeline */}
      <section>
        <div className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] shadow-[var(--shadow-card)] p-5">
          <h2 className="text-sm font-semibold text-[var(--color-text-primary)] mb-4">Enrollment Activity (30 days)</h2>
          {enrollError ? (
            <p className="text-sm text-[var(--color-error)]">Failed to load enrollment timeline.</p>
          ) : enrollLoading ? (
            <p className="text-sm text-[var(--color-text-tertiary)]">Loading enrollment timeline...</p>
          ) : !enrollmentTimeline || enrollmentTimeline.length === 0 ? (
            <p className="text-sm text-[var(--color-text-tertiary)] py-8 text-center">No enrollment data available.</p>
          ) : (
            <ResponsiveContainer width="100%" height={300}>
              <LineChart data={enrollmentTimeline} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-subtle)" />
                <XAxis
                  dataKey="date"
                  tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                  tickFormatter={(v: string) => {
                    const d = new Date(v);
                    return `${d.getMonth() + 1}/${d.getDate()}`;
                  }}
                />
                <YAxis
                  tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
                  allowDecimals={false}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: 'var(--color-surface)',
                    border: '1px solid var(--color-border-subtle)',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: 12,
                  }}
                />
                <Legend wrapperStyle={{ fontSize: 12 }} />
                <Line
                  type="monotone"
                  dataKey="enrolled"
                  name="Enrolled"
                  stroke="var(--color-success)"
                  strokeWidth={2}
                  dot={{ r: 3 }}
                />
                <Line
                  type="monotone"
                  dataKey="pending"
                  name="Pending"
                  stroke="var(--color-warning)"
                  strokeWidth={2}
                  dot={{ r: 3 }}
                />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
      </section>
    </div>
  );
}
