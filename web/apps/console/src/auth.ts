import { UserManager, WebStorageStateStore, type User } from 'oidc-client-ts';

// OIDC configuration — reads from Vite env vars with sensible dev defaults.
// In production, these are baked in at build time via VITE_* environment variables.
const OIDC_AUTHORITY = import.meta.env.VITE_OIDC_AUTHORITY as string | undefined
  ?? 'https://localhost:8443/oauth2/openid/hearth-console';
const OIDC_CLIENT_ID = import.meta.env.VITE_OIDC_CLIENT_ID as string | undefined
  ?? 'hearth-console';
const OIDC_REDIRECT_URI = import.meta.env.VITE_OIDC_REDIRECT_URI as string | undefined
  ?? `${window.location.origin}/console/callback`;
const OIDC_POST_LOGOUT_URI = import.meta.env.VITE_OIDC_POST_LOGOUT_URI as string | undefined
  ?? `${window.location.origin}/console/`;

export const userManager = new UserManager({
  authority: OIDC_AUTHORITY,
  client_id: OIDC_CLIENT_ID,
  redirect_uri: OIDC_REDIRECT_URI,
  post_logout_redirect_uri: OIDC_POST_LOGOUT_URI,
  response_type: 'code',
  scope: 'openid profile email groups',
  automaticSilentRenew: true,
  userStore: new WebStorageStateStore({ store: window.sessionStorage }),
});

/** Returns the current access token, or null if not authenticated. */
export async function getAccessToken(): Promise<string | null> {
  const user = await userManager.getUser();
  if (!user || user.expired) return null;
  return user.access_token;
}

/** Returns the current user profile, or null. */
export async function getUser(): Promise<User | null> {
  const user = await userManager.getUser();
  if (!user || user.expired) return null;
  return user;
}

/** Initiate login redirect. */
export function signIn() {
  return userManager.signinRedirect();
}

/** Initiate logout redirect. */
export function signOut() {
  return userManager.signoutRedirect();
}

/** Handle the OIDC callback — call this on the /callback route. */
export function handleCallback() {
  return userManager.signinRedirectCallback();
}

/** Check if auth is enabled (OIDC authority is configured). */
export function isAuthEnabled(): boolean {
  return !!OIDC_AUTHORITY;
}
