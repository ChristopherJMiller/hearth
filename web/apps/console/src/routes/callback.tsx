import { useEffect, useState } from 'react';
import { handleCallback } from '../auth';

/**
 * OIDC callback handler. Kanidm redirects here after authentication.
 * Completes the auth flow and redirects to the dashboard.
 */
export function CallbackPage() {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    handleCallback()
      .then(() => {
        window.location.replace('/console/dashboard');
      })
      .catch((err) => {
        console.error('OIDC callback failed:', err);
        setError(err instanceof Error ? err.message : 'Authentication failed');
      });
  }, []);

  if (error) {
    return (
      <div className="flex items-center justify-center h-screen bg-[var(--color-surface-base)]">
        <div className="text-center">
          <h1 className="text-lg font-semibold text-[var(--color-text-primary)] mb-2">
            Authentication Error
          </h1>
          <p className="text-sm text-[var(--color-text-secondary)] mb-4">{error}</p>
          <a
            href="/console/"
            className="text-sm text-[var(--color-ember)] hover:underline"
          >
            Return to console
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-center h-screen bg-[var(--color-surface-base)]">
      <p className="text-sm text-[var(--color-text-secondary)]">Completing sign-in...</p>
    </div>
  );
}
