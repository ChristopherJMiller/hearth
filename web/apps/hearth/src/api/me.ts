import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { UserConfig, UpdateMyConfigRequest } from './types';

export function useMyConfig() {
  return useQuery({
    queryKey: ['me', 'config'],
    queryFn: () => apiFetch<UserConfig>('/me/config'),
  });
}

export function useUpdateMyConfig() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateMyConfigRequest) =>
      apiFetch<UserConfig>('/me/config', {
        method: 'PUT',
        body: JSON.stringify(body),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['me', 'config'] });
    },
  });
}
