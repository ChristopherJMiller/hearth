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
      className="sticky top-0 z-20 flex items-center justify-between gap-4 bg-[var(--color-surface-base)]/85 backdrop-blur border-b border-[var(--color-border-subtle)]"
      style={{
        paddingLeft: 'var(--spacing-page-x)',
        paddingRight: 'var(--spacing-page-x)',
        paddingTop: '0.875rem',
        paddingBottom: '0.875rem',
      }}
    >
      <div className="min-w-0 flex-1">
        <Breadcrumbs items={crumbs} />
      </div>

      <div className="flex items-center gap-2 shrink-0">
        <button
          type="button"
          onClick={openCommandPalette}
          className="flex items-center gap-2.5 px-3 py-2 rounded-[var(--radius-sm)] bg-[var(--color-surface)] border border-[var(--color-border-subtle)] text-[var(--color-text-tertiary)] hover:text-[var(--color-text-secondary)] hover:border-[var(--color-border)] transition-colors cursor-pointer min-w-[260px]"
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
          className="relative flex items-center justify-center w-10 h-10 rounded-[var(--radius-sm)] bg-[var(--color-surface)] border border-[var(--color-border-subtle)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:border-[var(--color-border)] transition-colors cursor-pointer"
          aria-label="Notifications"
        >
          <LuBell size={16} />
          {hasNotifications && (
            <span
              className="absolute -top-1 -right-1 min-w-[18px] h-[18px] px-1 rounded-full bg-[var(--color-ember)] text-white flex items-center justify-center font-semibold text-2xs"
             
            >
              {notifCount}
            </span>
          )}
        </button>

        {enabled && (
          <div className="flex items-center gap-3 pl-2 ml-1 border-l border-[var(--color-border-subtle)]">
            <div className="flex flex-col items-end leading-tight">
              <span
                className="font-medium text-[var(--color-text-primary)] text-sm"
               
              >
                {displayName}
              </span>
              <span
                className="uppercase text-[var(--color-text-tertiary)] text-2xs tracking-wide"
               
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
