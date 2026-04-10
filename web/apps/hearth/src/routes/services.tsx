import {
  PageContainer,
  PageHeader,
  Card,
  Callout,
  SkeletonCard,
  EmptyState,
} from '@hearth/ui';
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

const categoryOrder: ServiceCategory[] = ['infrastructure', 'communication', 'storage', 'identity'];

const iconMap: Record<string, React.ComponentType<{ size?: number; className?: string }>> = {
  'message-square': LuMessageSquare,
  cloud: LuCloud,
  shield: LuShield,
  flame: LuFlame,
};

function ServiceIcon({ icon }: { icon: string | null }) {
  const Icon = (icon && iconMap[icon]) || LuGlobe;
  return <Icon size={22} className="text-[var(--color-ember)]" />;
}

export function ServicesPage() {
  const { data: services, isLoading, isError } = useServices();

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Observability"
        title="Services"
        description="Platform services exposed by the Hearth Home cluster — chat, storage, identity, and infrastructure tooling."
      />

      {isError ? (
        <Callout variant="danger" title="Could not load services" />
      ) : isLoading ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-[var(--spacing-card-gap)]">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
      ) : !services || services.length === 0 ? (
        <EmptyState
          icon={<LuGlobe size={28} />}
          title="No services configured"
          description="Enable capabilities in your Helm values (chat, cloud, observability…) to surface services here."
        />
      ) : (
        <div className="flex flex-col gap-[var(--spacing-section)]">
          {categoryOrder
            .map((cat) => ({
              category: cat,
              label: categoryLabels[cat],
              items: services.filter((s) => s.category === cat),
            }))
            .filter((g) => g.items.length > 0)
            .map((group) => (
              <section key={group.category} className="flex flex-col gap-3">
                <h2
                  className="uppercase font-semibold text-[var(--color-text-tertiary)] text-2xs tracking-wide"
                 
                >
                  {group.label}
                </h2>
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-[var(--spacing-card-gap)]">
                  {group.items.map((service) => (
                    <a
                      key={service.id}
                      href={service.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block group"
                    >
                      <Card className="h-full hover:border-[var(--color-border-accent)] transition-colors">
                        <div className="flex items-start gap-4">
                          <div
                            className="shrink-0 w-12 h-12 rounded-[var(--radius-md)] flex items-center justify-center"
                            style={{ background: 'var(--color-ember-faint)' }}
                          >
                            <ServiceIcon icon={service.icon} />
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <h3
                                className="font-semibold text-[var(--color-text-primary)] group-hover:text-[var(--color-ember)] transition-colors text-base"
                               
                              >
                                {service.name}
                              </h3>
                              <LuExternalLink
                                size={14}
                                className="text-[var(--color-text-tertiary)] opacity-0 group-hover:opacity-100 transition-opacity"
                              />
                            </div>
                            {service.description && (
                              <p
                                className="text-[var(--color-text-secondary)] mt-1.5 text-sm leading-body"
                               
                              >
                                {service.description}
                              </p>
                            )}
                            <div
                              className="flex items-center gap-1.5 mt-3 text-[var(--color-text-tertiary)] font-mono truncate text-2xs"
                             
                            >
                              {new URL(service.url).hostname}
                            </div>
                          </div>
                        </div>
                      </Card>
                    </a>
                  ))}
                </div>
              </section>
            ))}
        </div>
      )}
    </PageContainer>
  );
}
