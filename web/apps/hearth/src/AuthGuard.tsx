import { useEffect, useState, type ReactNode } from 'react';
import { userManager, signIn, isAuthEnabled } from './auth';
import type { User } from 'oidc-client-ts';

interface AuthGuardProps {
  children: ReactNode;
}

/**
 * Wraps the application and ensures the user is authenticated before
 * rendering children. When OIDC is not configured (dev without Kanidm),
 * renders children immediately.
 */
export function AuthGuard({ children }: AuthGuardProps) {
  const [state, setState] = useState<'loading' | 'authenticated' | 'unauthenticated'>('loading');

  useEffect(() => {
    if (!isAuthEnabled()) {
      setState('authenticated');
      return;
    }

    userManager.getUser().then((u) => {
      if (u && !u.expired) {
        setState('authenticated');
      } else {
        setState('unauthenticated');
      }
    });

    const onUserLoaded = (_u: User) => setState('authenticated');
    userManager.events.addUserLoaded(onUserLoaded);

    return () => {
      userManager.events.removeUserLoaded(onUserLoaded);
    };
  }, []);

  useEffect(() => {
    if (state === 'unauthenticated') {
      signIn();
    }
  }, [state]);

  if (state === 'loading') {
    return (
      <div className="flex items-center justify-center h-screen bg-surface-base">
        <p className="text-sm text-text-secondary">Loading...</p>
      </div>
    );
  }

  if (state === 'unauthenticated') {
    return (
      <div className="flex items-center justify-center h-screen bg-surface-base">
        <p className="text-sm text-text-secondary">Redirecting to sign in...</p>
      </div>
    );
  }

  return <>{children}</>;
}
