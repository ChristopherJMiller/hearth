import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { ComplianceReport, DeploymentTimelineEntry, EnrollmentTimelineEntry } from './types';

export function useComplianceReport() {
  return useQuery({
    queryKey: ['reports', 'compliance'],
    queryFn: () => apiFetch<ComplianceReport>('/reports/compliance'),
    refetchInterval: 30_000,
  });
}

export function useDeploymentTimeline(days = 30) {
  return useQuery({
    queryKey: ['reports', 'deployments', days],
    queryFn: () => apiFetch<DeploymentTimelineEntry[]>(`/reports/deployments?days=${days}`),
  });
}

export function useEnrollmentTimeline(days = 30) {
  return useQuery({
    queryKey: ['reports', 'enrollments', days],
    queryFn: () => apiFetch<EnrollmentTimelineEntry[]>(`/reports/enrollments?days=${days}`),
  });
}
