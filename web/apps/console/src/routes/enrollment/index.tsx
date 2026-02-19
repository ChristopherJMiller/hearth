import { useState } from 'react';
import { PageHeader, Card, Select, Button, EmptyState } from '@hearth/ui';
import { usePendingEnrollments, useApproveEnrollment } from '../../api/enrollment';
import { formatRelativeTime } from '../../lib/time';
import { LuUserPlus, LuFingerprint, LuClock, LuCheckCircle, LuChevronDown } from 'react-icons/lu';

const roleOptions = [
  { value: 'default', label: 'Default' },
  { value: 'developer', label: 'Developer' },
  { value: 'designer', label: 'Designer' },
  { value: 'admin', label: 'Admin' },
];

function EnrollmentCard({ machine }: { machine: { id: string; hostname: string; hardware_fingerprint: string | null; created_at: string } }) {
  const [role, setRole] = useState('default');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [targetClosure, setTargetClosure] = useState('');
  const [cacheUrl, setCacheUrl] = useState('');
  const approve = useApproveEnrollment();

  const handleApprove = () => {
    approve.mutate({
      id: machine.id,
      role,
      admin: 'console-admin',
      target_closure: targetClosure || undefined,
      cache_url: cacheUrl || undefined,
    });
  };

  return (
    <Card>
      <div className="flex flex-col gap-4">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-[var(--radius-sm)] bg-[var(--color-warning-faint)] flex items-center justify-center text-[var(--color-warning)]">
              <LuUserPlus size={20} />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
                {machine.hostname}
              </h3>
              <span className="inline-flex items-center gap-1.5 text-xs font-medium px-2 py-0.5 rounded-full bg-[var(--color-warning-faint)] text-[var(--color-warning)] mt-1">
                <span className="w-1.5 h-1.5 rounded-full bg-[var(--color-warning)] animate-[pulse-dot_1.8s_ease-in-out_infinite]" />
                Pending
              </span>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
          <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
            <LuFingerprint size={14} className="text-[var(--color-text-tertiary)] shrink-0" />
            <span className="font-mono text-xs truncate">
              {machine.hardware_fingerprint ?? 'Unknown'}
            </span>
          </div>
          <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
            <LuClock size={14} className="text-[var(--color-text-tertiary)] shrink-0" />
            <span>{formatRelativeTime(machine.created_at)}</span>
          </div>
        </div>

        <div className="flex items-end gap-3 pt-2 border-t border-[var(--color-border-subtle)]">
          <Select
            options={roleOptions}
            value={role}
            onChange={setRole}
            label="Assign Role"
            className="flex-1"
          />
          <Button
            variant="primary"
            size="sm"
            onClick={handleApprove}
            disabled={approve.isPending}
          >
            <LuCheckCircle size={14} />
            {approve.isPending ? 'Approving...' : 'Approve'}
          </Button>
        </div>

        <button
          type="button"
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="flex items-center gap-1 text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] transition-colors"
        >
          <LuChevronDown size={12} className={showAdvanced ? 'rotate-180' : ''} style={{ transition: 'transform 0.2s' }} />
          Advanced options
        </button>

        {showAdvanced && (
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">Target Closure</label>
              <input
                type="text"
                value={targetClosure}
                onChange={(e) => setTargetClosure(e.target.value)}
                placeholder="e.g. /nix/store/...-nixos-system-hearth"
                className="w-full px-3 py-1.5 text-sm rounded-[var(--radius-sm)] bg-[var(--color-surface-base)] border border-[var(--color-border-subtle)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-tertiary)] focus:outline-none focus:border-[var(--color-ember)]"
              />
            </div>
            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium text-[var(--color-text-secondary)]">Cache URL</label>
              <input
                type="text"
                value={cacheUrl}
                onChange={(e) => setCacheUrl(e.target.value)}
                placeholder="e.g. http://localhost:8080/hearth"
                className="w-full px-3 py-1.5 text-sm rounded-[var(--radius-sm)] bg-[var(--color-surface-base)] border border-[var(--color-border-subtle)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-tertiary)] focus:outline-none focus:border-[var(--color-ember)]"
              />
            </div>
          </div>
        )}

        {approve.isError && (
          <p className="text-xs text-[var(--color-error)]">
            Failed to approve enrollment. Please try again.
          </p>
        )}
      </div>
    </Card>
  );
}

export function EnrollmentPage() {
  const { data: pending, isLoading } = usePendingEnrollments();

  return (
    <div>
      <PageHeader
        title="Enrollment"
        description="Review and approve pending device enrollments"
      />

      {isLoading ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading pending enrollments...
        </p>
      ) : !pending || pending.length === 0 ? (
        <EmptyState
          icon={<LuUserPlus size={24} />}
          title="No pending enrollments"
          description="All devices have been reviewed. New devices will appear here when they request enrollment."
        />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {pending.map((machine) => (
            <EnrollmentCard key={machine.id} machine={machine} />
          ))}
        </div>
      )}
    </div>
  );
}
