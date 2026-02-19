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
    mutationFn: ({ id, role, admin }: { id: string; role: string; admin: string }) =>
      apiFetch<Machine>(`/machines/${id}/approve`, {
        method: 'POST',
        body: JSON.stringify({ role, admin }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['machines'] });
      qc.invalidateQueries({ queryKey: ['stats'] });
    },
  });
}
