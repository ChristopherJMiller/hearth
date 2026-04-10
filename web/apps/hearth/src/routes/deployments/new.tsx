import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  TextInput,
  Button,
  Card,
  Callout,
  KeyValueEditor,
  DescriptionList,
} from '@hearth/ui';
import { useCreateDeployment } from '../../api/deployments';
import { LuRocket, LuArrowLeft, LuTarget, LuPackage, LuSettings } from 'react-icons/lu';

export function NewDeploymentPage() {
  const router = useRouter();
  const createDeployment = useCreateDeployment();

  const [closure, setClosure] = useState('');
  const [moduleLibraryRef, setModuleLibraryRef] = useState('');
  const [canarySize, setCanarySize] = useState('1');
  const [batchSize, setBatchSize] = useState('5');
  const [failureThreshold, setFailureThreshold] = useState('0.1');
  const [targetFilter, setTargetFilter] = useState<Record<string, string>>({});

  const [errors, setErrors] = useState<Record<string, string>>({});

  const validate = (): boolean => {
    const next: Record<string, string> = {};
    if (!closure.trim()) {
      next.closure = 'Closure path is required';
    }
    const cs = Number(canarySize);
    if (isNaN(cs) || cs < 0 || !Number.isInteger(cs)) {
      next.canarySize = 'Must be a non-negative integer';
    }
    const bs = Number(batchSize);
    if (isNaN(bs) || bs < 1 || !Number.isInteger(bs)) {
      next.batchSize = 'Must be a positive integer';
    }
    const ft = Number(failureThreshold);
    if (isNaN(ft) || ft < 0 || ft > 1) {
      next.failureThreshold = 'Must be between 0 and 1';
    }
    setErrors(next);
    return Object.keys(next).length === 0;
  };

  const handleSubmit = () => {
    if (!validate()) return;

    createDeployment.mutate(
      {
        closure: closure.trim(),
        module_library_ref: moduleLibraryRef.trim() || undefined,
        canary_size: Number(canarySize),
        batch_size: Number(batchSize),
        failure_threshold: Number(failureThreshold),
        target_filter: Object.keys(targetFilter).length > 0 ? targetFilter : undefined,
      },
      {
        onSuccess: (deployment) => {
          router.navigate({
            to: '/deployments/$deploymentId',
            params: { deploymentId: deployment.id },
          });
        },
      },
    );
  };

  return (
    <PageContainer size="default">
      <PageHeader
        eyebrow="New rollout"
        title="New deployment"
        description="Build a phased rollout: pick a closure, narrow the target fleet, and dial in canary safety."
        breadcrumbs={[
          { label: 'Software' },
          { label: 'Deployments', onClick: () => router.navigate({ to: '/deployments' }) },
          { label: 'New' },
        ]}
      />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-[var(--spacing-card-gap)] items-start">
        <div className="lg:col-span-2 flex flex-col gap-[var(--spacing-card-gap)]">
          <Card>
            <div className="flex items-center gap-2 mb-5">
              <LuPackage size={16} className="text-[var(--color-text-tertiary)]" />
              <h2 className="font-semibold text-[var(--color-text-primary)] text-lg">
                Closure
              </h2>
            </div>
            <div className="flex flex-col gap-4">
              <TextInput
                label="Closure path"
                value={closure}
                onChange={setClosure}
                placeholder="/nix/store/...-nixos-system-24.05"
                error={errors.closure}
              />
              <TextInput
                label="Module library reference (optional)"
                value={moduleLibraryRef}
                onChange={setModuleLibraryRef}
                placeholder="e.g. github:org/fleet-modules#main"
              />
            </div>
          </Card>

          <Card>
            <div className="flex items-center gap-2 mb-2">
              <LuTarget size={16} className="text-[var(--color-text-tertiary)]" />
              <h2 className="font-semibold text-[var(--color-text-primary)] text-lg">
                Target filter
              </h2>
            </div>
            <p className="text-[var(--color-text-tertiary)] mb-4 text-xs">
              Optional. Restrict the rollout by tag or role. Empty matches the full fleet.
            </p>
            <KeyValueEditor
              value={targetFilter}
              onChange={setTargetFilter}
              keyLabel="Field"
              valueLabel="Value"
              keyPlaceholder="role / tag"
              valuePlaceholder="developer"
              addLabel="Add filter"
              emptyLabel="No filters — rollout will target the entire fleet."
            />
          </Card>

          <Card>
            <div className="flex items-center gap-2 mb-5">
              <LuSettings size={16} className="text-[var(--color-text-tertiary)]" />
              <h2 className="font-semibold text-[var(--color-text-primary)] text-lg">
                Rollout policy
              </h2>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <TextInput
                label="Canary size"
                value={canarySize}
                onChange={setCanarySize}
                placeholder="1"
                error={errors.canarySize}
              />
              <TextInput
                label="Batch size"
                value={batchSize}
                onChange={setBatchSize}
                placeholder="5"
                error={errors.batchSize}
              />
              <TextInput
                label="Failure threshold"
                value={failureThreshold}
                onChange={setFailureThreshold}
                placeholder="0.1"
                error={errors.failureThreshold}
              />
            </div>
            <p className="mt-3 text-[var(--color-text-tertiary)] text-xs">
              The failure threshold is the fraction of machines (0 to 1) that can fail before the deployment is halted.
            </p>
          </Card>

          {createDeployment.isError && (
            <Callout variant="danger" title="Could not create deployment">
              Check your closure path and try again.
            </Callout>
          )}

          <div className="flex justify-end gap-3">
            <Button
              variant="ghost"
              leadingIcon={<LuArrowLeft size={14} />}
              onClick={() => router.navigate({ to: '/deployments' })}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              size="lg"
              loading={createDeployment.isPending}
              leadingIcon={<LuRocket size={15} />}
              onClick={handleSubmit}
            >
              Launch deployment
            </Button>
          </div>
        </div>

        <div>
          <Card>
            <div className="mb-4">
              <h2 className="font-semibold text-[var(--color-text-primary)] text-lg">
                Review
              </h2>
              <p className="text-[var(--color-text-tertiary)] text-xs">
                What you're about to launch
              </p>
            </div>
            <DescriptionList
              columns={1}
              items={[
                {
                  label: 'Closure',
                  value: closure ? <span className="font-mono break-all text-2xs">{closure}</span> : <span className="italic text-[var(--color-text-tertiary)]">unset</span>,
                },
                {
                  label: 'Module library',
                  value: moduleLibraryRef || <span className="italic text-[var(--color-text-tertiary)]">default</span>,
                },
                {
                  label: 'Filter',
                  value: Object.keys(targetFilter).length === 0
                    ? <span className="italic text-[var(--color-text-tertiary)]">whole fleet</span>
                    : (
                      <div className="flex flex-col gap-1">
                        {Object.entries(targetFilter).map(([k, v]) => (
                          <span key={k} className="font-mono text-xs">
                            {k}: <span className="text-[var(--color-text-secondary)]">{v}</span>
                          </span>
                        ))}
                      </div>
                    ),
                },
                { label: 'Canary', value: `${canarySize} machine${Number(canarySize) === 1 ? '' : 's'}` },
                { label: 'Batch', value: `${batchSize} machine${Number(batchSize) === 1 ? '' : 's'}` },
                { label: 'Failure threshold', value: `${(Number(failureThreshold) * 100 || 0).toFixed(0)}%` },
              ]}
            />
          </Card>
        </div>
      </div>
    </PageContainer>
  );
}
