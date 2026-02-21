import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { FleetStats } from './types';

export function useFleetStats() {
  return useQuery({
    queryKey: ['stats'],
    queryFn: () => apiFetch<FleetStats>('/stats'),
    refetchInterval: 15000,
  });
}
