import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';
import { PageHeader, TextInput, Button, Card } from '@hearth/ui';
import { useCreateDeployment } from '../../api/deployments';
import { LuRocket } from 'react-icons/lu';

export function NewDeploymentPage() {
  const router = useRouter();
  const createDeployment = useCreateDeployment();

  const [closure, setClosure] = useState('');
  const [moduleLibraryRef, setModuleLibraryRef] = useState('');
  const [canarySize, setCanarySize] = useState('1');
  const [batchSize, setBatchSize] = useState('5');
  const [failureThreshold, setFailureThreshold] = useState('0.1');

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
    <div>
      <PageHeader
        title="New Deployment"
        description="Deploy a NixOS closure across the fleet"
        breadcrumbs={[
          { label: 'Deployments', onClick: () => router.navigate({ to: '/deployments' }) },
          { label: 'New' },
        ]}
      />

      <Card className="max-w-2xl">
        <div className="flex flex-col gap-5">
          <TextInput
            label="Closure Path"
            value={closure}
            onChange={setClosure}
            placeholder="/nix/store/...-nixos-system-24.05"
            error={errors.closure}
          />

          <TextInput
            label="Module Library Reference (optional)"
            value={moduleLibraryRef}
            onChange={setModuleLibraryRef}
            placeholder="e.g. github:org/fleet-modules#main"
          />

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-5">
            <TextInput
              label="Canary Size"
              value={canarySize}
              onChange={setCanarySize}
              placeholder="1"
              error={errors.canarySize}
            />

            <TextInput
              label="Batch Size"
              value={batchSize}
              onChange={setBatchSize}
              placeholder="5"
              error={errors.batchSize}
            />

            <TextInput
              label="Failure Threshold"
              value={failureThreshold}
              onChange={setFailureThreshold}
              placeholder="0.1"
              error={errors.failureThreshold}
            />
          </div>

          <p className="text-xs text-[var(--color-text-tertiary)]">
            The failure threshold is the fraction of machines (0 to 1) that can fail before the deployment is automatically halted.
          </p>

          {createDeployment.isError && (
            <p className="text-sm text-[var(--color-error)]">
              Failed to create deployment. Please check your inputs and try again.
            </p>
          )}

          <div className="flex justify-end gap-3 pt-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => router.navigate({ to: '/deployments' })}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleSubmit}
              disabled={createDeployment.isPending}
            >
              <LuRocket size={14} />
              {createDeployment.isPending ? 'Creating...' : 'Create Deployment'}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
