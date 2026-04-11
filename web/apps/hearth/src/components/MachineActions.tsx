import { useState } from 'react';
import { Button, ConfirmDialog } from '@hearth/ui';
import { useMachineActions, useCreateAction } from '../api/actions';
import { useRoles } from '../hooks/useRoles';
import { formatRelativeTime } from '../lib/time';
import type { ActionType, ActionStatus } from '../api/types';
import { LuLock, LuRotateCcw, LuHammer, LuChevronDown, LuChevronRight } from 'react-icons/lu';

interface MachineActionsProps {
  machineId: string;
}

const actionStatusColors: Record<ActionStatus, string> = {
  pending: 'bg-warning-faint text-warning',
  delivered: 'bg-info-faint text-info',
  running: 'bg-info-faint text-info',
  completed: 'bg-success-faint text-success',
  failed: 'bg-error-faint text-error',
};

const actionLabels: Record<ActionType, string> = {
  lock: 'Lock',
  restart: 'Restart',
  rebuild: 'Rebuild',
  run_command: 'Run Command',
};

interface ConfirmState {
  open: boolean;
  actionType: ActionType | null;
}

export function MachineActions({ machineId }: MachineActionsProps) {
  const { data: actions, isLoading, error } = useMachineActions(machineId);
  const createAction = useCreateAction(machineId);
  const { isOperator } = useRoles();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<ConfirmState>({ open: false, actionType: null });

  const handleTrigger = (actionType: ActionType) => {
    setConfirm({ open: true, actionType });
  };

  const handleConfirm = () => {
    if (confirm.actionType) {
      createAction.mutate({ action_type: confirm.actionType });
    }
    setConfirm({ open: false, actionType: null });
  };

  return (
    <div>
      {/* Action buttons */}
      {isOperator && (
        <div className="flex items-center gap-2 mb-4">
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTrigger('lock')}
            disabled={createAction.isPending}
          >
            <LuLock size={14} />
            Lock
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTrigger('restart')}
            disabled={createAction.isPending}
          >
            <LuRotateCcw size={14} />
            Restart
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTrigger('rebuild')}
            disabled={createAction.isPending}
          >
            <LuHammer size={14} />
            Rebuild
          </Button>
        </div>
      )}

      {/* Confirm Dialog */}
      <ConfirmDialog
        open={confirm.open}
        onOpenChange={(open: boolean) => setConfirm((prev) => ({ ...prev, open }))}
        title={`Confirm ${confirm.actionType ? actionLabels[confirm.actionType] : ''}`}
        description={`Are you sure you want to ${confirm.actionType ? actionLabels[confirm.actionType].toLowerCase() : ''} this machine? This action will be sent to the agent on its next heartbeat.`}
        confirmLabel={confirm.actionType ? actionLabels[confirm.actionType] : 'Confirm'}
        variant="danger"
        onConfirm={handleConfirm}
      />

      {/* Actions list */}
      <div className="bg-surface border border-border-subtle rounded-md shadow-card">
        <div className="px-5 py-4 border-b border-border-subtle">
          <h2 className="text-sm font-semibold text-text-primary">Recent Actions</h2>
        </div>

        {error ? (
          <p className="text-sm text-error px-5 py-8 text-center">
            Failed to load actions.
          </p>
        ) : isLoading ? (
          <p className="text-sm text-text-tertiary px-5 py-8 text-center">
            Loading actions...
          </p>
        ) : !actions || actions.length === 0 ? (
          <p className="text-sm text-text-tertiary px-5 py-8 text-center">
            No actions have been triggered for this machine.
          </p>
        ) : (
          <div className="divide-y divide-border-subtle">
            {actions.map((action) => {
              const isExpanded = expandedId === action.id;
              return (
                <div key={action.id}>
                  <button
                    type="button"
                    className="flex items-center justify-between w-full px-5 py-3 hover:bg-surface-raised transition-colors cursor-pointer text-left"
                    onClick={() => setExpandedId(isExpanded ? null : action.id)}
                  >
                    <div className="flex items-center gap-4 min-w-0">
                      <span className="text-sm font-medium text-text-primary whitespace-nowrap">
                        {actionLabels[action.action_type] ?? action.action_type}
                      </span>
                      <span
                        className={`inline-flex items-center gap-1.5 text-xs font-medium px-2.5 py-0.5 rounded-full whitespace-nowrap ${actionStatusColors[action.status] ?? ''}`}
                      >
                        {action.status}
                      </span>
                      <span className="text-xs text-text-tertiary truncate">
                        {action.created_by ?? 'system'}
                      </span>
                      <span className="text-xs text-text-tertiary whitespace-nowrap">
                        {formatRelativeTime(action.created_at)}
                      </span>
                    </div>
                    {action.result ? (
                      isExpanded ? (
                        <LuChevronDown size={14} className="text-text-tertiary shrink-0" />
                      ) : (
                        <LuChevronRight size={14} className="text-text-tertiary shrink-0" />
                      )
                    ) : null}
                  </button>
                  {isExpanded && action.result && (
                    <div className="px-5 pb-4">
                      <pre className="text-xs font-mono bg-surface-base border border-border-subtle rounded-sm p-3 overflow-x-auto text-text-secondary">
                        {JSON.stringify(action.result, null, 2)}
                      </pre>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Mutation error feedback */}
      {createAction.isError && (
        <p className="text-sm text-error mt-2">
          Failed to create action: {createAction.error?.message ?? 'Unknown error'}
        </p>
      )}
    </div>
  );
}
