import { PageHeader, Card } from '@hearth/ui';
import {
  LuMessageSquare,
  LuCloud,
  LuShield,
  LuFlame,
  LuExternalLink,
  LuGlobe,
} from 'react-icons/lu';
import { useServices } from '../api/services';
import type { ServiceCategory } from '../api/types';

const categoryLabels: Record<ServiceCategory, string> = {
  communication: 'Communication',
  storage: 'Storage',
  identity: 'Identity',
  infrastructure: 'Infrastructure',
};

const categoryOrder: ServiceCategory[] = [
  'infrastructure',
  'communication',
  'storage',
  'identity',
];

function ServiceIcon({ icon }: { icon: string | null }) {
  const size = 28;
  const className = "text-[var(--color-ember)]";
  switch (icon) {
    case 'message-square':
      return <LuMessageSquare size={size} className={className} />;
    case 'cloud':
      return <LuCloud size={size} className={className} />;
    case 'shield':
      return <LuShield size={size} className={className} />;
    case 'flame':
      return <LuFlame size={size} className={className} />;
    default:
      return <LuGlobe size={size} className={className} />;
  }
}

export function ServicesPage() {
  const { data: services, isLoading, error } = useServices();

  if (isLoading) {
    return (
      <div className="p-6">
        <PageHeader title="Services" description="Loading available services..." />
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <PageHeader title="Services" description="Failed to load services." />
        <p className="text-sm text-[var(--color-ember)] mt-4">{error.message}</p>
      </div>
    );
  }

  if (!services || services.length === 0) {
    return (
      <div className="p-6">
        <PageHeader
          title="Services"
          description="No services are currently configured. Enable capabilities in your Helm values to add services."
        />
      </div>
    );
  }

  // Group by category, preserving display order
  const grouped = categoryOrder
    .map((cat) => ({
      category: cat,
      label: categoryLabels[cat],
      items: services.filter((s) => s.category === cat),
    }))
    .filter((g) => g.items.length > 0);

  return (
    <div className="p-6 space-y-8">
      <PageHeader
        title="Services"
        description="Platform services available to your organization."
      />

      {grouped.map((group) => (
        <div key={group.category} className="space-y-3">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--color-text-muted)]">
            {group.label}
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {group.items.map((service) => (
              <a
                key={service.id}
                href={service.url}
                target="_blank"
                rel="noopener noreferrer"
                className="block group"
              >
                <Card>
                  <div className="p-5 flex items-start gap-4">
                    <div className="shrink-0 mt-0.5">
                      <ServiceIcon icon={service.icon} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <h3 className="text-sm font-semibold text-[var(--color-text-primary)] group-hover:text-[var(--color-ember)] transition-colors">
                          {service.name}
                        </h3>
                        <LuExternalLink size={14} className="text-[var(--color-text-muted)] opacity-0 group-hover:opacity-100 transition-opacity" />
                      </div>
                      {service.description && (
                        <p className="text-xs text-[var(--color-text-secondary)] mt-1 leading-relaxed">
                          {service.description}
                        </p>
                      )}
                    </div>
                  </div>
                </Card>
              </a>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
