import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { Deployment, DeploymentMachineStatus } from './types';

export function useDeployments(status?: string) {
  const params = status ? `?status=${status}` : '';
  return useQuery({
    queryKey: ['deployments', status],
    queryFn: () => apiFetch<Deployment[]>(`/deployments${params}`),
  });
}

export function useDeployment(id: string) {
  return useQuery({
    queryKey: ['deployments', id],
    queryFn: () => apiFetch<Deployment>(`/deployments/${id}`),
    enabled: !!id,
    refetchInterval: 5000,
  });
}

export function useDeploymentMachines(id: string) {
  return useQuery({
    queryKey: ['deployments', id, 'machines'],
    queryFn: () => apiFetch<DeploymentMachineStatus[]>(`/deployments/${id}/machines`),
    enabled: !!id,
    refetchInterval: 5000,
  });
}

export function useCreateDeployment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      closure: string;
      module_library_ref?: string;
      target_filter?: Record<string, unknown>;
      canary_size?: number;
      batch_size?: number;
      failure_threshold?: number;
    }) => apiFetch<Deployment>('/deployments', { method: 'POST', body: JSON.stringify(body) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['deployments'] });
    },
  });
}

export function useRollbackDeployment() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<Deployment>(`/deployments/${id}/rollback`, { method: 'POST' }),
    onSuccess: (_, id) => {
      qc.invalidateQueries({ queryKey: ['deployments'] });
      qc.invalidateQueries({ queryKey: ['deployments', id] });
    },
  });
}
