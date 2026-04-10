import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { BuildJob, BuildJobStatus } from './types';

export function useBuildJobs(status?: BuildJobStatus) {
  const qs = status ? `?status=${status}` : '';
  return useQuery({
    queryKey: ['build-jobs', status ?? 'all'],
    queryFn: () => apiFetch<BuildJob[]>(`/build-jobs${qs}`),
    refetchInterval: 5000,
  });
}

export function useBuildJob(id: string) {
  return useQuery({
    queryKey: ['build-jobs', id],
    queryFn: () => apiFetch<BuildJob>(`/build-jobs/${id}`),
    enabled: !!id,
    refetchInterval: 3000,
  });
}
