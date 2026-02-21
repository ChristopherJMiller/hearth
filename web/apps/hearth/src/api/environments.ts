import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { UserEnvironment } from './types';

export function useMachineEnvironments(machineId: string) {
  return useQuery({
    queryKey: ['machines', machineId, 'environments'],
    queryFn: () =>
      apiFetch<UserEnvironment[]>(`/machines/${machineId}/environments`),
    enabled: !!machineId,
  });
}
