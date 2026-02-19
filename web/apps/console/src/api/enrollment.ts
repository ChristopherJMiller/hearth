import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { Machine } from './types';

export function usePendingEnrollments() {
  return useQuery({
    queryKey: ['machines', 'pending'],
    queryFn: async () => {
      const machines = await apiFetch<Machine[]>('/machines');
      return machines.filter((m) => m.enrollment_status === 'pending');
    },
    refetchInterval: 10000,
  });
}

export function useApproveEnrollment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, role, admin, target_closure, cache_url }: {
      id: string; role: string; admin: string;
      target_closure?: string; cache_url?: string;
    }) =>
      apiFetch<Machine>(`/machines/${id}/approve`, {
        method: 'POST',
        body: JSON.stringify({ role, admin, target_closure, cache_url }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['machines'] });
      qc.invalidateQueries({ queryKey: ['stats'] });
    },
  });
}
