import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type {
  DriftedMachine,
  DriftStatus,
  CompliancePolicy,
  CreateCompliancePolicyRequest,
  PolicyResult,
  DeploymentComplianceSummary,
  DeploymentSbom,
} from './types';

// --- Drift ---

export function useDriftedMachines(status?: DriftStatus | 'all') {
  const params = status && status !== 'all' ? `?status=${status}` : '';
  return useQuery({
    queryKey: ['compliance', 'drift', status ?? 'all'],
    queryFn: () => apiFetch<DriftedMachine[]>(`/compliance/drift${params}`),
    refetchInterval: 30_000,
  });
}

// --- Policies ---

export function useCompliancePolicies() {
  return useQuery({
    queryKey: ['compliance', 'policies'],
    queryFn: () => apiFetch<CompliancePolicy[]>('/compliance/policies'),
  });
}

export function useCreatePolicy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateCompliancePolicyRequest) =>
      apiFetch<CompliancePolicy>('/compliance/policies', {
        method: 'POST',
        body: JSON.stringify(req),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['compliance', 'policies'] }),
  });
}

export function useUpdatePolicy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: string } & Partial<CreateCompliancePolicyRequest>) =>
      apiFetch<CompliancePolicy>(`/compliance/policies/${id}`, {
        method: 'PUT',
        body: JSON.stringify(body),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['compliance', 'policies'] }),
  });
}

export function useDeletePolicy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<{ deleted: boolean }>(`/compliance/policies/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['compliance', 'policies'] }),
  });
}

// --- Deployment results ---

export function useDeploymentPolicyResults(deploymentId: string) {
  return useQuery({
    queryKey: ['compliance', 'deployments', deploymentId, 'results'],
    queryFn: () => apiFetch<PolicyResult[]>(`/compliance/deployments/${deploymentId}/results`),
    enabled: !!deploymentId,
  });
}

export function useDeploymentComplianceSummary(deploymentId: string) {
  return useQuery({
    queryKey: ['compliance', 'deployments', deploymentId, 'summary'],
    queryFn: () =>
      apiFetch<DeploymentComplianceSummary>(`/compliance/deployments/${deploymentId}/summary`),
    enabled: !!deploymentId,
  });
}

// --- SBOMs ---

export function useDeploymentSboms(deploymentId: string) {
  return useQuery({
    queryKey: ['compliance', 'sboms', deploymentId],
    queryFn: () => apiFetch<DeploymentSbom[]>(`/compliance/sboms/${deploymentId}`),
    enabled: !!deploymentId,
  });
}
