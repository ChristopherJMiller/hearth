import { useState } from 'react';
import {
  PageContainer,
  PageHeader,
  Card,
  Select,
  Button,
  EmptyState,
  StatusChip,
  Sheet,
  TextInput,
  Callout,
  SkeletonCard,
  Tooltip,
} from '@hearth/ui';
import { usePendingEnrollments, useApproveEnrollment } from '../../api/enrollment';
import { useActor } from '../../hooks/useActor';
import { formatRelativeTime } from '../../lib/time';
import {
  LuUserPlus,
  LuFingerprint,
  LuClock,
  LuCheckCircle,
  LuSettings2,
  LuCopy,
} from 'react-icons/lu';

const roleOptions = [
  { value: 'default', label: 'Default' },
  { value: 'developer', label: 'Developer' },
  { value: 'designer', label: 'Designer' },
  { value: 'admin', label: 'Admin' },
];

interface PendingMachine {
  id: string;
  hostname: string;
  hardware_fingerprint: string | null;
  created_at: string;
}

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Tooltip content={copied ? 'Copied!' : 'Copy'}>
      <button
        type="button"
        onClick={() => {
          navigator.clipboard.writeText(value);
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        }}
        className="inline-flex items-center justify-center w-7 h-7 rounded-[6px] text-text-tertiary hover:text-text-primary hover:bg-surface-raised cursor-pointer"
        aria-label="Copy"
      >
        <LuCopy size={13} />
      </button>
    </Tooltip>
  );
}

function EnrollmentCard({ machine }: { machine: PendingMachine }) {
  const [role, setRole] = useState('default');
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [targetClosure, setTargetClosure] = useState('');
  const [cacheUrl, setCacheUrl] = useState('');
  const approve = useApproveEnrollment();
  const actor = useActor();

  const handleApprove = (extra?: { target_closure?: string; cache_url?: string }) => {
    approve.mutate(
      {
        id: machine.id,
        role,
        admin: actor,
        target_closure: extra?.target_closure || undefined,
        cache_url: extra?.cache_url || undefined,
      },
      {
        onSuccess: () => setAdvancedOpen(false),
      },
    );
  };

  return (
    <>
      <Card>
        <div className="flex flex-col gap-5">
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="shrink-0 w-12 h-12 rounded-md flex items-center justify-center text-warning bg-warning-faint">
                <LuUserPlus size={22} />
              </div>
              <div className="flex flex-col gap-1 min-w-0">
                <h3
                  className="font-semibold text-text-primary truncate text-lg"
                 
                >
                  {machine.hostname}
                </h3>
                <StatusChip status="pending" />
              </div>
            </div>
          </div>

          <div className="flex flex-col gap-3 p-4 rounded-sm bg-surface-sunken border border-border-subtle">
            <div className="flex items-center gap-2">
              <LuFingerprint size={14} className="text-text-tertiary shrink-0" />
              <span
                className="uppercase font-semibold text-text-tertiary text-2xs tracking-wide"
               
              >
                Hardware fingerprint
              </span>
            </div>
            <div className="flex items-center justify-between gap-2">
              <code
                className="font-mono text-text-primary break-all flex-1 text-xs"
               
              >
                {machine.hardware_fingerprint ?? 'unknown'}
              </code>
              {machine.hardware_fingerprint && <CopyButton value={machine.hardware_fingerprint} />}
            </div>
            <div
              className="flex items-center gap-1.5 text-text-tertiary text-2xs"
             
            >
              <LuClock size={12} />
              Requested {formatRelativeTime(machine.created_at)}
            </div>
          </div>

          <div className="flex items-end gap-3">
            <Select
              options={roleOptions}
              value={role}
              onChange={setRole}
              label="Assign role"
              className="flex-1"
            />
            <Button
              variant="primary"
              loading={approve.isPending}
              leadingIcon={<LuCheckCircle size={14} />}
              onClick={() => handleApprove()}
            >
              Approve
            </Button>
          </div>

          <button
            type="button"
            onClick={() => setAdvancedOpen(true)}
            className="self-start inline-flex items-center gap-1.5 text-text-tertiary hover:text-text-secondary cursor-pointer text-xs"
           
          >
            <LuSettings2 size={12} />
            Advanced options…
          </button>

          {approve.isError && (
            <Callout variant="danger" title="Approval failed">
              The control plane returned an error. Please try again.
            </Callout>
          )}
        </div>
      </Card>

      <Sheet
        open={advancedOpen}
        onOpenChange={setAdvancedOpen}
        title="Advanced enrollment"
        description={`${machine.hostname} · ${role}`}
        size="md"
        footer={
          <div className="flex items-center justify-end gap-2">
            <Button variant="ghost" onClick={() => setAdvancedOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              loading={approve.isPending}
              leadingIcon={<LuCheckCircle size={14} />}
              onClick={() =>
                handleApprove({
                  target_closure: targetClosure || undefined,
                  cache_url: cacheUrl || undefined,
                })
              }
            >
              Approve with overrides
            </Button>
          </div>
        }
      >
        <div className="flex flex-col gap-5">
          <TextInput
            label="Target closure"
            value={targetClosure}
            onChange={setTargetClosure}
            placeholder="/nix/store/...-nixos-system-hearth"
          />
          <TextInput
            label="Cache URL"
            value={cacheUrl}
            onChange={setCacheUrl}
            placeholder="http://localhost:8080/hearth"
          />
        </div>
      </Sheet>
    </>
  );
}

export function EnrollmentPage() {
  const { data: pending, isLoading, isError } = usePendingEnrollments();

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Fleet"
        title="Enrollment"
        description="Devices that have requested to join the fleet. Approve to issue an enrollment token and start provisioning."
      />

      {isError ? (
        <Callout variant="danger" title="Could not load enrollments" />
      ) : isLoading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-card-gap">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
      ) : !pending || pending.length === 0 ? (
        <EmptyState
          icon={<LuUserPlus size={28} />}
          title="No pending enrollments"
          description="All devices have been reviewed. New devices will appear here when they request enrollment."
        />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-card-gap">
          {pending.map((machine) => (
            <EnrollmentCard key={machine.id} machine={machine} />
          ))}
        </div>
      )}
    </PageContainer>
  );
}
