import { useEffect, useState } from 'react';
import { userManager, signOut as doSignOut, isAuthEnabled } from './auth';
import type { User } from 'oidc-client-ts';

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const enabled = isAuthEnabled();

  useEffect(() => {
    if (!enabled) return;

    userManager.getUser().then((u) => {
      if (u && !u.expired) setUser(u);
    });

    const onUserLoaded = (u: User) => setUser(u);
    const onUserUnloaded = () => setUser(null);

    userManager.events.addUserLoaded(onUserLoaded);
    userManager.events.addUserUnloaded(onUserUnloaded);

    return () => {
      userManager.events.removeUserLoaded(onUserLoaded);
      userManager.events.removeUserUnloaded(onUserUnloaded);
    };
  }, [enabled]);

  return { user, signOut: doSignOut, enabled };
}
