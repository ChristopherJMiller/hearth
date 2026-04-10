import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { Machine } from './types';

/**
 * Shares the `/machines` cache with `useMachines` (same `queryKey`) so
 * mounting both hooks on one page doesn't fetch `/machines` twice.
 * The `select` filter runs client-side and is memoized by React Query.
 */
export function usePendingEnrollments() {
  return useQuery({
    queryKey: ['machines'],
    queryFn: () => apiFetch<Machine[]>('/machines'),
    select: (machines) => machines.filter((m) => m.enrollment_status === 'pending'),
    refetchInterval: 10000,
  });
}

export function useApproveEnrollment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, role, admin, target_closure, cache_url, disko_config }: {
      id: string; role: string; admin: string;
      target_closure?: string; cache_url?: string; disko_config?: string;
    }) =>
      apiFetch<Machine>(`/machines/${id}/approve`, {
        method: 'POST',
        body: JSON.stringify({ role, admin, target_closure, cache_url, disko_config }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['machines'] });
      qc.invalidateQueries({ queryKey: ['stats'] });
    },
  });
}
