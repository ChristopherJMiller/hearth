import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { CreateActionRequest, PendingAction } from './types';

export function useMachineActions(machineId: string) {
  return useQuery({
    queryKey: ['machines', machineId, 'actions'],
    queryFn: () => apiFetch<PendingAction[]>(`/machines/${machineId}/actions`),
  });
}

/** One-shot POST for use outside React components (command palette, etc.). */
export function createMachineAction(machineId: string, req: CreateActionRequest) {
  return apiFetch<PendingAction>(`/machines/${machineId}/actions`, {
    method: 'POST',
    body: JSON.stringify(req),
  });
}

export function useCreateAction(machineId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateActionRequest) => createMachineAction(machineId, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['machines', machineId, 'actions'] });
    },
  });
}
