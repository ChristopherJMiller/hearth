import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { CreateActionRequest, PendingAction } from './types';

export function useMachineActions(machineId: string) {
  return useQuery({
    queryKey: ['machines', machineId, 'actions'],
    queryFn: () => apiFetch<PendingAction[]>(`/machines/${machineId}/actions`),
  });
}

export function useCreateAction(machineId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateActionRequest) =>
      apiFetch<PendingAction>(`/machines/${machineId}/actions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['machines', machineId, 'actions'] });
    },
  });
}
