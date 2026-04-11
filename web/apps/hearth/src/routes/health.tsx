import {
  PageContainer,
  PageHeader,
  MetricTile,
  Callout,
  SkeletonCard,
  Card,
} from '@hearth/ui';
import { useFleetStats } from '../api/stats';
import { useServices } from '../api/services';
import { useDeployments } from '../api/deployments';
import { LuActivity, LuMonitor, LuLayers, LuCloud, LuShield, LuFlame, LuMessageSquare, LuGlobe } from 'react-icons/lu';
import type { ServiceCategory } from '../api/types';

const categoryIcon: Record<ServiceCategory, React.ReactNode> = {
  identity: <LuShield size={18} />,
  communication: <LuMessageSquare size={18} />,
  storage: <LuCloud size={18} />,
  infrastructure: <LuFlame size={18} />,
};

export function HealthPage() {
  const stats = useFleetStats();
  const services = useServices();
  const deployments = useDeployments();

  const activeRollouts = (deployments.data ?? []).filter(
    (d) => d.status === 'canary' || d.status === 'rolling',
  ).length;
  const failedRollouts = (deployments.data ?? []).filter((d) => d.status === 'failed').length;

  const apiHealthy = !stats.isError;
  const servicesHealthy = !services.isError && (services.data?.length ?? 0) > 0;

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Observability"
        title="System health"
        description="A bird's-eye view of every Hearth subsystem. Green means happy. Anything else needs your attention."
      />

      {!apiHealthy && (
        <div className="mb-6">
          <Callout variant="danger" title="Control plane unreachable">
            We can't reach the Hearth API. Verify <code className="font-mono">hearth-api</code> is running.
          </Callout>
        </div>
      )}

      <section>
        <h2
          className="uppercase font-semibold text-text-tertiary mb-3 text-2xs tracking-wide"
         
        >
          Core
        </h2>
        <div
          className="grid gap-card-gap"
          style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
        >
          <MetricTile
            label="API"
            value={apiHealthy ? 'Healthy' : 'Down'}
            icon={<LuActivity size={18} />}
            tone={apiHealthy ? 'success' : 'danger'}
          />
          <MetricTile
            label="Active machines"
            value={stats.data?.active_machines ?? '—'}
            sublabel={stats.data ? `${stats.data.total_machines} total` : undefined}
            icon={<LuMonitor size={18} />}
            tone={stats.data && stats.data.active_machines > 0 ? 'success' : 'warning'}
          />
          <MetricTile
            label="Active rollouts"
            value={activeRollouts}
            icon={<LuLayers size={18} />}
            tone={activeRollouts > 0 ? 'info' : 'default'}
          />
          <MetricTile
            label="Failed rollouts"
            value={failedRollouts}
            icon={<LuLayers size={18} />}
            tone={failedRollouts > 0 ? 'danger' : 'default'}
          />
        </div>
      </section>

      <section>
        <h2
          className="uppercase font-semibold text-text-tertiary mb-3 text-2xs tracking-wide"
         
        >
          Services
        </h2>
        {services.isError ? (
          <Callout variant="danger" title="Services discovery failed" />
        ) : services.isLoading ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-card-gap">
            <SkeletonCard />
            <SkeletonCard />
            <SkeletonCard />
          </div>
        ) : !services.data || services.data.length === 0 ? (
          <Card>
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-sm flex items-center justify-center text-text-tertiary bg-surface-raised">
                <LuGlobe size={18} />
              </div>
              <div>
                <div className="font-medium text-text-primary text-sm">
                  No services configured
                </div>
                <div className="text-text-tertiary text-xs">
                  Enable capabilities in your Helm values to surface services.
                </div>
              </div>
            </div>
          </Card>
        ) : (
          <div
            className="grid gap-card-gap"
            style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}
          >
            {services.data.map((service) => (
              <MetricTile
                key={service.id}
                label={service.category}
                value={service.name}
                icon={categoryIcon[service.category] ?? <LuGlobe size={18} />}
                tone={servicesHealthy ? 'success' : 'danger'}
                onClick={() => window.open(service.url, '_blank', 'noopener')}
              />
            ))}
          </div>
        )}
      </section>
    </PageContainer>
  );
}
