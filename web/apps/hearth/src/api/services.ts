import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { ServiceInfo } from './types';

export function useServices() {
  return useQuery({
    queryKey: ['services'],
    queryFn: () => apiFetch<ServiceInfo[]>('/services'),
    staleTime: 5 * 60 * 1000,
  });
}
