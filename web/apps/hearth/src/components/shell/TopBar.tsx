import { useRouter } from '@tanstack/react-router';
import { Breadcrumbs, Avatar, Kbd } from '@hearth/ui';
import { LuSearch, LuBell } from 'react-icons/lu';
import { useAuth } from '../../useAuth';
import { useRoles } from '../../hooks/useRoles';
import { useActor } from '../../hooks/useActor';
import { useNotifications } from '../../hooks/useNotifications';
import { buildBreadcrumbs } from './navConfig';
import { useShell } from './ShellContext';

export function TopBar() {
  const router = useRouter();
  const pathname = router.state.location.pathname;
  const { enabled } = useAuth();
  const { role } = useRoles();
  const { openCommandPalette, toggleNotifications } = useShell();
  const { hasUnread, items: notifications } = useNotifications();
  const actor = useActor();

  const crumbs = buildBreadcrumbs(pathname).map((c) => ({
    label: c.label,
    onClick: c.path ? () => router.navigate({ to: c.path }) : undefined,
  }));

  const displayName = enabled ? actor : 'Guest';

  const hasNotifications = hasUnread;
  const notifCount = notifications.length;

  return (
    <header
      className="sticky top-0 z-20 flex items-center justify-between gap-4 bg-surface-base/85 backdrop-blur border-b border-border-subtle px-page-x py-3.5"
    >
      <div className="min-w-0 flex-1">
        <Breadcrumbs items={crumbs} />
      </div>

      <div className="flex items-center gap-2 shrink-0">
        <button
          type="button"
          onClick={openCommandPalette}
          className="flex items-center gap-2.5 px-3 py-2 rounded-sm bg-surface border border-border-subtle text-text-tertiary hover:text-text-secondary hover:border-border transition-colors cursor-pointer min-w-[260px]"
          aria-label="Search or run command"
        >
          <LuSearch size={15} />
          <span className="flex-1 text-left text-sm">
            Search or jump to…
          </span>
          <Kbd>⌘K</Kbd>
        </button>

        <button
          type="button"
          onClick={toggleNotifications}
          className="relative flex items-center justify-center w-10 h-10 rounded-sm bg-surface border border-border-subtle text-text-secondary hover:text-text-primary hover:border-border transition-colors cursor-pointer"
          aria-label="Notifications"
        >
          <LuBell size={16} />
          {hasNotifications && (
            <span
              className="absolute -top-1 -right-1 min-w-[18px] h-[18px] px-1 rounded-full bg-ember text-white flex items-center justify-center font-semibold text-2xs"
             
            >
              {notifCount}
            </span>
          )}
        </button>

        {enabled && (
          <div className="flex items-center gap-3 pl-2 ml-1 border-l border-border-subtle">
            <div className="flex flex-col items-end leading-tight">
              <span
                className="font-medium text-text-primary text-sm"
               
              >
                {displayName}
              </span>
              <span
                className="uppercase text-text-tertiary text-2xs tracking-wide"
               
              >
                {role}
              </span>
            </div>
            <Avatar name={displayName} size="sm" />
          </div>
        )}
      </div>
    </header>
  );
}
