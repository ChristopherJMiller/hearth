import { getAccessToken, signIn, isAuthEnabled } from '../auth';

const BASE = '/api/v1';

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init?.headers as Record<string, string>),
  };

  // Attach Bearer token when OIDC auth is enabled
  if (isAuthEnabled()) {
    const token = await getAccessToken();
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
  }

  const resp = await fetch(`${BASE}${path}`, { ...init, headers });

  if (resp.status === 401 && isAuthEnabled()) {
    // Token expired or invalid — redirect to login
    await signIn();
    throw new ApiError(401, 'Session expired, redirecting to login...');
  }

  if (!resp.ok) {
    const body = await resp.text().catch(() => '');
    throw new ApiError(resp.status, body || resp.statusText);
  }
  if (resp.status === 204) return undefined as T;
  return resp.json();
}
