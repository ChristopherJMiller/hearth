import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from './client';
import type { CatalogEntry, SoftwareRequest, SoftwareRequestStatus } from './types';

export function useCatalog() {
  return useQuery({
    queryKey: ['catalog'],
    queryFn: () => apiFetch<CatalogEntry[]>('/catalog'),
  });
}

export function useCreateCatalogEntry() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: Omit<CatalogEntry, 'id' | 'created_at'>) =>
      apiFetch<CatalogEntry>('/catalog', { method: 'POST', body: JSON.stringify(body) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['catalog'] });
    },
  });
}

export function useUpdateCatalogEntry() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: { id: string } & Partial<CatalogEntry>) =>
      apiFetch<CatalogEntry>(`/catalog/${id}`, { method: 'PUT', body: JSON.stringify(body) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['catalog'] });
    },
  });
}

export function useDeleteCatalogEntry() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<void>(`/catalog/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['catalog'] });
    },
  });
}

// --- User-facing catalog hooks ---

export function useRequests() {
  return useQuery({
    queryKey: ['requests'],
    queryFn: () => apiFetch<SoftwareRequest[]>('/requests'),
  });
}

interface RequestSoftwareVars {
  catalogEntryId: string;
  machineId: string;
  username: string;
}

export function useRequestSoftware() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ catalogEntryId, machineId, username }: RequestSoftwareVars) =>
      apiFetch<SoftwareRequest>(`/catalog/${catalogEntryId}/request`, {
        method: 'POST',
        body: JSON.stringify({ machine_id: machineId, username }),
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['catalog'] });
      qc.invalidateQueries({ queryKey: ['requests'] });
    },
  });
}

/**
 * Find the most recent request status for a catalog entry by a specific
 * user on a specific machine. Returns the status string or null.
 */
export function getRequestStatus(
  requests: SoftwareRequest[] | undefined,
  catalogEntryId: string,
  machineId: string,
  username: string,
): SoftwareRequestStatus | null {
  if (!requests) return null;

  const matching = requests.filter(
    (r) =>
      r.catalog_entry_id === catalogEntryId &&
      r.machine_id === machineId &&
      r.username === username,
  );

  if (matching.length === 0) return null;

  matching.sort(
    (a, b) => new Date(b.requested_at).getTime() - new Date(a.requested_at).getTime(),
  );

  return matching[0].status;
}
