import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { Machine } from './types';

export function useMachines() {
  return useQuery({
    queryKey: ['machines'],
    queryFn: () => apiFetch<Machine[]>('/machines'),
  });
}

export function useMachine(id: string) {
  return useQuery({
    queryKey: ['machines', id],
    queryFn: () => apiFetch<Machine>(`/machines/${id}`),
    enabled: !!id,
  });
}

export function useUpdateMachine() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: string } & Partial<Machine>) =>
      apiFetch<Machine>(`/machines/${id}`, { method: 'PUT', body: JSON.stringify(body) }),
    onSuccess: (_, vars) => {
      qc.invalidateQueries({ queryKey: ['machines'] });
      qc.invalidateQueries({ queryKey: ['machines', vars.id] });
    },
  });
}

export function useDeleteMachine() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<void>(`/machines/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['machines'] });
    },
  });
}
