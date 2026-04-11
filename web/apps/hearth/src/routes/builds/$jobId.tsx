import { useRouter, useParams } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  Card,
  StatusChip,
  DescriptionList,
  Callout,
  SkeletonCard,
  Button,
} from '@hearth/ui';
import { useBuildJob } from '../../api/builds';
import { formatDateTime, truncateStorePath, truncateId } from '../../lib/time';
import { LuArrowLeft, LuHammer, LuRocket } from 'react-icons/lu';

export function BuildJobDetailPage() {
  const router = useRouter();
  const { jobId } = useParams({ strict: false }) as { jobId: string };
  const { data: job, isLoading, isError } = useBuildJob(jobId);

  if (isError) {
    return (
      <PageContainer>
        <Callout variant="danger" title="Build job not found" />
      </PageContainer>
    );
  }

  if (isLoading || !job) {
    return (
      <PageContainer>
        <PageHeader title="Loading build job…" />
        <SkeletonCard />
      </PageContainer>
    );
  }

  return (
    <PageContainer size="default">
      <PageHeader
        eyebrow={truncateId(job.id)}
        title={truncateStorePath(job.flake_ref)}
        description={`Worker · ${job.worker_id ?? 'unclaimed'}`}
        breadcrumbs={[
          { label: 'Software' },
          { label: 'Build queue', onClick: () => router.navigate({ to: '/builds' }) },
          { label: truncateId(job.id) },
        ]}
        actions={
          <>
            <Button
              variant="ghost"
              leadingIcon={<LuArrowLeft size={14} />}
              onClick={() => router.navigate({ to: '/builds' })}
            >
              Back
            </Button>
            {job.deployment_id && (
              <Button
                variant="primary"
                leadingIcon={<LuRocket size={15} />}
                onClick={() =>
                  router.navigate({
                    to: '/deployments/$deploymentId',
                    params: { deploymentId: job.deployment_id! },
                  })
                }
              >
                Open deployment
              </Button>
            )}
          </>
        }
      />

      <div className="flex flex-col gap-card-gap">
        <Card>
          <div className="flex items-center gap-3 mb-5">
            <LuHammer size={16} className="text-text-tertiary" />
            <h2 className="font-semibold text-text-primary text-lg">
              Status
            </h2>
            <div className="ml-auto"><StatusChip status={job.status} /></div>
          </div>

          <DescriptionList
            columns={2}
            items={[
              { label: 'Flake reference', value: <span className="font-mono break-all text-xs">{job.flake_ref}</span>, span: 2 },
              { label: 'Closure', value: job.closure ? <span className="font-mono break-all text-xs">{job.closure}</span> : <span className="italic text-text-tertiary">not yet built</span>, span: 2 },
              { label: 'Closures built', value: job.closures_built ?? '—' },
              { label: 'Closures pushed', value: job.closures_pushed ?? '—' },
              { label: 'Total machines', value: job.total_machines ?? '—' },
              { label: 'Worker', value: job.worker_id ?? <span className="italic text-text-tertiary">unclaimed</span> },
              { label: 'Canary', value: `${job.canary_size}` },
              { label: 'Batch', value: `${job.batch_size}` },
              { label: 'Failure threshold', value: `${(job.failure_threshold * 100).toFixed(0)}%` },
              { label: 'Claimed at', value: job.claimed_at ? formatDateTime(job.claimed_at) : <span className="italic text-text-tertiary">never</span> },
              { label: 'Created', value: formatDateTime(job.created_at) },
              { label: 'Updated', value: formatDateTime(job.updated_at) },
            ]}
          />

          {job.error_message && (
            <div className="mt-5">
              <Callout variant="danger" title="Build error">
                <pre className="font-mono whitespace-pre-wrap text-xs">
                  {job.error_message}
                </pre>
              </Callout>
            </div>
          )}
        </Card>

        <Callout variant="info" title="Logs coming soon">
          Streaming build worker logs is not yet exposed by the control plane. This view will
          render the live log feed once the backend supports it.
        </Callout>
      </div>
    </PageContainer>
  );
}
