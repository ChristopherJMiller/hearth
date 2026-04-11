import {
  PageContainer,
  PageHeader,
  Card,
  StatusChip,
  Avatar,
  DescriptionList,
  Callout,
  SkeletonCard,
  Button,
} from '@hearth/ui';
import { useMyConfig } from '../../api/me';
import { useActor } from '../../hooks/useActor';
import { useRouter } from '@tanstack/react-router';
import { LuSettings, LuUser } from 'react-icons/lu';
import { formatDateTime } from '../../lib/time';

export function MyEnvironmentPage() {
  const { data: config, isLoading, isError } = useMyConfig();
  const router = useRouter();
  const displayName = useActor();

  if (isError) {
    return (
      <PageContainer size="default">
        <Callout variant="danger" title="Could not load your environment" />
      </PageContainer>
    );
  }

  if (isLoading || !config) {
    return (
      <PageContainer size="default">
        <PageHeader title="My environment" />
        <SkeletonCard />
      </PageContainer>
    );
  }

  return (
    <PageContainer size="default">
      <PageHeader
        eyebrow="Personal"
        title="My environment"
        description="Your per-user closure — base role, build status, and where to tweak it."
        actions={
          <Button
            variant="primary"
            leadingIcon={<LuSettings size={15} />}
            onClick={() => router.navigate({ to: '/settings' })}
          >
            Edit settings
          </Button>
        }
      />

      <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-card-gap items-start">
        <Card>
          <div className="flex flex-col items-center gap-3 text-center">
            <Avatar name={displayName} size="lg" />
            <div className="flex flex-col gap-1">
              <h2
                className="font-semibold text-text-primary text-lg"
               
              >
                {displayName}
              </h2>
              <span
                className="text-text-tertiary capitalize text-xs"
               
              >
                {config.base_role} role
              </span>
            </div>
            <StatusChip status={config.build_status} />
          </div>
        </Card>

        <Card>
          <div className="flex items-center gap-2 mb-5">
            <LuUser size={16} className="text-text-tertiary" />
            <h2
              className="font-semibold text-text-primary text-lg"
             
            >
              Closure details
            </h2>
          </div>
          <DescriptionList
            columns={2}
            items={[
              { label: 'Username', value: config.username },
              { label: 'Base role', value: <span className="capitalize">{config.base_role}</span> },
              { label: 'Build status', value: <StatusChip status={config.build_status} /> },
              { label: 'Latest closure', value: config.latest_closure ? <span className="font-mono break-all text-2xs">{config.latest_closure}</span> : <span className="italic text-text-tertiary">none</span>, span: 2 },
              { label: 'Created', value: formatDateTime(config.created_at) },
              { label: 'Updated', value: formatDateTime(config.updated_at) },
            ]}
          />
          {config.build_error && (
            <div className="mt-5">
              <Callout variant="danger" title="Build error">
                <pre className="font-mono whitespace-pre-wrap text-xs">
                  {config.build_error}
                </pre>
              </Callout>
            </div>
          )}
        </Card>
      </div>
    </PageContainer>
  );
}
