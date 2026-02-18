import {
  useQuery,
  useMutation,
  useQueryClient,
  type UseQueryResult,
} from '@tanstack/react-query';
import type { CatalogEntry, SoftwareRequest, SoftwareRequestStatus } from './types';

async function fetchJson<T>(url: string): Promise<T> {
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  }
  return resp.json() as Promise<T>;
}

export function useCatalog(): UseQueryResult<CatalogEntry[]> {
  return useQuery({
    queryKey: ['catalog'],
    queryFn: () => fetchJson<CatalogEntry[]>('/api/v1/catalog'),
  });
}

export function useRequests(): UseQueryResult<SoftwareRequest[]> {
  return useQuery({
    queryKey: ['requests'],
    queryFn: () => fetchJson<SoftwareRequest[]>('/api/v1/requests'),
  });
}

interface RequestSoftwareVars {
  catalogEntryId: string;
  machineId: string;
  username: string;
}

export function useRequestSoftware() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ catalogEntryId, machineId, username }: RequestSoftwareVars) => {
      const resp = await fetch(`/api/v1/catalog/${catalogEntryId}/request`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ machine_id: machineId, username }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({})) as Record<string, string>;
        throw new Error(err.error || `HTTP ${resp.status}`);
      }
      return resp.json() as Promise<SoftwareRequest>;
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['catalog'] });
      void queryClient.invalidateQueries({ queryKey: ['requests'] });
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

  // Sort by requested_at descending and return the latest
  matching.sort(
    (a, b) => new Date(b.requested_at).getTime() - new Date(a.requested_at).getTime(),
  );

  return matching[0].status;
}
