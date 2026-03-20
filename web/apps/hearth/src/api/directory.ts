import { useQuery } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { DirectoryPerson } from './types';

export function useDirectory() {
  return useQuery({
    queryKey: ['directory', 'people'],
    queryFn: () => apiFetch<DirectoryPerson[]>('/directory/people'),
    staleTime: 5 * 60 * 1000,
  });
}
