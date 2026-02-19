import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { SoftwareRequest } from './types';

export function useSoftwareRequests(status?: string) {
  const params = status ? `?status=${status}` : '';
  return useQuery({
    queryKey: ['requests', status],
    queryFn: () => apiFetch<SoftwareRequest[]>(`/requests${params}`),
  });
}

export function usePendingRequests() {
  return useSoftwareRequests('pending');
}

export function useApproveRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, admin }: { id: string; admin: string }) =>
      apiFetch<SoftwareRequest>(`/requests/${id}/approve`, {
        method: 'POST',
        body: JSON.stringify({ admin }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['requests'] });
      qc.invalidateQueries({ queryKey: ['stats'] });
    },
  });
}

export function useDenyRequest() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, admin }: { id: string; admin: string }) =>
      apiFetch<SoftwareRequest>(`/requests/${id}/deny`, {
        method: 'POST',
        body: JSON.stringify({ admin }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['requests'] });
      qc.invalidateQueries({ queryKey: ['stats'] });
    },
  });
}
