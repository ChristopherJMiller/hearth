import { useRouter } from '@tanstack/react-router';
import { useFleetStats } from '../../api/stats';
import { useServices } from '../../api/services';

interface PulseDotProps {
  label: string;
  tone: 'success' | 'warning' | 'error' | 'idle';
}

function PulseDot({ label, tone }: PulseDotProps) {
  const color =
    tone === 'success'
      ? 'var(--color-success)'
      : tone === 'warning'
        ? 'var(--color-warning)'
        : tone === 'error'
          ? 'var(--color-error)'
          : 'var(--color-text-tertiary)';
  return (
    <div className="flex items-center gap-2">
      <span
        className="w-2 h-2 rounded-full shrink-0 animate-[pulse-dot_1.8s_ease-in-out_infinite]"
        style={{ background: color }}
      />
      <span
        className="text-text-secondary truncate text-2xs tracking-wide"
       
      >
        {label}
      </span>
    </div>
  );
}

export function SystemPulse() {
  const router = useRouter();
  const stats = useFleetStats();
  const services = useServices();

  const apiTone: PulseDotProps['tone'] = stats.isError
    ? 'error'
    : stats.isLoading
      ? 'idle'
      : 'success';
  const servicesTone: PulseDotProps['tone'] = services.isError
    ? 'error'
    : services.isLoading
      ? 'idle'
      : 'success';
  const fleetTone: PulseDotProps['tone'] = stats.data
    ? stats.data.active_machines === 0
      ? 'warning'
      : 'success'
    : 'idle';

  return (
    <button
      type="button"
      onClick={() => router.navigate({ to: '/health' })}
      className="flex flex-col gap-1.5 px-4 py-3 border-t border-border-subtle cursor-pointer hover:bg-surface-raised transition-colors w-full text-left"
      title="System health"
    >
      <div
        className="font-semibold uppercase text-text-tertiary text-2xs tracking-wide"
       
      >
        System Pulse
      </div>
      <div className="flex flex-col gap-1">
        <PulseDot label="API" tone={apiTone} />
        <PulseDot label="Services" tone={servicesTone} />
        <PulseDot label="Fleet" tone={fleetTone} />
      </div>
    </button>
  );
}
