import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { AuditEvent } from './types';

export function useAuditLog(filters?: {
  event_type?: string;
  machine_id?: string;
  actor?: string;
  limit?: number;
}) {
  const params = new URLSearchParams();
  if (filters?.event_type) params.set('event_type', filters.event_type);
  if (filters?.machine_id) params.set('machine_id', filters.machine_id);
  if (filters?.actor) params.set('actor', filters.actor);
  if (filters?.limit) params.set('limit', String(filters.limit));
  const qs = params.toString();
  return useQuery({
    queryKey: ['audit', filters],
    queryFn: () => apiFetch<AuditEvent[]>(`/audit${qs ? `?${qs}` : ''}`),
  });
}
