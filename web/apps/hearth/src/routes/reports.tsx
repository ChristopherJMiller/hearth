import { PageContainer, PageHeader, MetricTile, Card, Callout, SkeletonCard } from '@hearth/ui';
import {
  useComplianceReport,
  useDeploymentTimeline,
  useEnrollmentTimeline,
} from '../api/reports';
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
import { LuMonitor, LuShieldCheck, LuShieldAlert, LuShieldQuestion } from 'react-icons/lu';
import {
  chartTooltipContent,
  chartAxisTick,
  chartGridStroke,
} from '../components/charts/ChartTooltip';

const formatDateTick = (v: string): string => {
  const d = new Date(v);
  return `${d.getMonth() + 1}/${d.getDate()}`;
};

export function ReportsPage() {
  const compliance = useComplianceReport();
  const deploymentTimeline = useDeploymentTimeline();
  const enrollmentTimeline = useEnrollmentTimeline();

  const compliancePct =
    compliance.data && compliance.data.total > 0
      ? Math.round((compliance.data.compliant / compliance.data.total) * 100)
      : 0;

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Observability"
        title="Reports"
        description="Long-running fleet metrics — compliance posture, deployment cadence, and enrollment flow over time."
      />

      <section className="mb-[var(--spacing-section)]">
        <h2
          className="uppercase font-semibold text-[var(--color-text-tertiary)] mb-3 text-2xs tracking-wide"
         
        >
          Compliance posture
        </h2>
        {compliance.isError ? (
          <Callout variant="danger" title="Could not load compliance data" />
        ) : compliance.isLoading ? (
          <SkeletonCard />
        ) : (
          <div
            className="grid gap-[var(--spacing-card-gap)]"
            style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
          >
            <MetricTile
              label="Total machines"
              value={compliance.data?.total ?? 0}
              icon={<LuMonitor size={18} />}
              tone="ember"
            />
            <MetricTile
              label="Compliant"
              value={compliance.data?.compliant ?? 0}
              sublabel={compliance.data && compliance.data.total > 0 ? `${compliancePct}%` : undefined}
              icon={<LuShieldCheck size={18} />}
              tone="success"
            />
            <MetricTile
              label="Drifted"
              value={compliance.data?.drifted ?? 0}
              icon={<LuShieldAlert size={18} />}
              tone={compliance.data && compliance.data.drifted > 0 ? 'danger' : 'default'}
            />
            <MetricTile
              label="No target"
              value={compliance.data?.no_target ?? 0}
              icon={<LuShieldQuestion size={18} />}
              tone="default"
            />
          </div>
        )}
      </section>

      <section className="mb-[var(--spacing-section)]">
        <Card>
          <h2
            className="font-semibold text-[var(--color-text-primary)] mb-1 text-lg"
           
          >
            Deployment activity
          </h2>
          <p
            className="text-[var(--color-text-tertiary)] mb-5 text-xs"
           
          >
            Last 30 days
          </p>
          {deploymentTimeline.isError ? (
            <Callout variant="danger" title="Could not load deployment timeline" />
          ) : deploymentTimeline.isLoading ? (
            <div className="h-[280px]" />
          ) : !deploymentTimeline.data || deploymentTimeline.data.length === 0 ? (
            <p
              className="text-[var(--color-text-tertiary)] py-12 text-center text-sm"
             
            >
              No deployment data yet.
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={280}>
              <BarChart data={deploymentTimeline.data} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
                <CartesianGrid strokeDasharray="3 3" stroke={chartGridStroke} />
                <XAxis dataKey="date" tick={chartAxisTick} tickFormatter={formatDateTick} />
                <YAxis tick={chartAxisTick} allowDecimals={false} />
                <Tooltip contentStyle={chartTooltipContent} cursor={{ fill: 'var(--color-surface-raised)' }} />
                <Legend wrapperStyle={{ fontSize: 12, color: 'var(--color-text-secondary)' }} />
                <Bar dataKey="completed" name="Completed" fill="var(--chart-3)" radius={[3, 3, 0, 0]} />
                <Bar dataKey="failed" name="Failed" fill="var(--chart-1)" radius={[3, 3, 0, 0]} />
                <Bar dataKey="rolled_back" name="Rolled back" fill="var(--chart-4)" radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </Card>
      </section>

      <section>
        <Card>
          <h2
            className="font-semibold text-[var(--color-text-primary)] mb-1 text-lg"
           
          >
            Enrollment activity
          </h2>
          <p
            className="text-[var(--color-text-tertiary)] mb-5 text-xs"
           
          >
            Last 30 days
          </p>
          {enrollmentTimeline.isError ? (
            <Callout variant="danger" title="Could not load enrollment timeline" />
          ) : enrollmentTimeline.isLoading ? (
            <div className="h-[280px]" />
          ) : !enrollmentTimeline.data || enrollmentTimeline.data.length === 0 ? (
            <p
              className="text-[var(--color-text-tertiary)] py-12 text-center text-sm"
             
            >
              No enrollment data yet.
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={280}>
              <LineChart data={enrollmentTimeline.data} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
                <CartesianGrid strokeDasharray="3 3" stroke={chartGridStroke} />
                <XAxis dataKey="date" tick={chartAxisTick} tickFormatter={formatDateTick} />
                <YAxis tick={chartAxisTick} allowDecimals={false} />
                <Tooltip contentStyle={chartTooltipContent} />
                <Legend wrapperStyle={{ fontSize: 12, color: 'var(--color-text-secondary)' }} />
                <Line
                  type="monotone"
                  dataKey="enrolled"
                  name="Enrolled"
                  stroke="var(--chart-3)"
                  strokeWidth={2}
                  dot={{ r: 3, fill: 'var(--chart-3)' }}
                />
                <Line
                  type="monotone"
                  dataKey="pending"
                  name="Pending"
                  stroke="var(--chart-4)"
                  strokeWidth={2}
                  dot={{ r: 3, fill: 'var(--chart-4)' }}
                />
              </LineChart>
            </ResponsiveContainer>
          )}
        </Card>
      </section>
    </PageContainer>
  );
}
