import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { UserConfig } from './types';

export function useUserConfig(username: string) {
  return useQuery({
    queryKey: ['users', username, 'config'],
    queryFn: () => apiFetch<UserConfig>(`/users/${username}/config`),
    enabled: !!username,
  });
}

export interface UpsertUserConfigRequest {
  base_role: string;
  overrides?: Record<string, unknown>;
}

export function useUpdateUserConfig(username: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: UpsertUserConfigRequest) =>
      apiFetch<UserConfig>(`/users/${username}/config`, {
        method: 'PUT',
        body: JSON.stringify(body),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['users', username] });
    },
  });
}

export function useRebuildUserConfig(username: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () =>
      apiFetch<unknown>(`/users/${username}/config/build`, {
        method: 'POST',
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['users', username] });
    },
  });
}
