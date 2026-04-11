import { useEffect, useRef } from 'react';
import { useRouter } from '@tanstack/react-router';
import { useShell } from './ShellContext';
import { useNotifications, type Notification } from '../../hooks/useNotifications';
import { LuBell, LuChevronRight } from 'react-icons/lu';

const toneBg: Record<Notification['tone'], string> = {
  info: 'var(--color-info-faint)',
  success: 'var(--color-success-faint)',
  warning: 'var(--color-warning-faint)',
  danger: 'var(--color-error-faint)',
};
const toneFg: Record<Notification['tone'], string> = {
  info: 'var(--color-info)',
  success: 'var(--color-success)',
  warning: 'var(--color-warning)',
  danger: 'var(--color-error)',
};

export function NotificationCenter() {
  const router = useRouter();
  const { notificationsOpen, closeNotifications } = useShell();
  const { items } = useNotifications();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!notificationsOpen) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        closeNotifications();
      }
    };
    // Slight delay so the click that opened it doesn't immediately close it.
    const t = setTimeout(() => document.addEventListener('mousedown', onClick), 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('mousedown', onClick);
    };
  }, [notificationsOpen, closeNotifications]);

  if (!notificationsOpen) return null;

  return (
    <div
      ref={ref}
      className="fixed right-6 top-[68px] z-40 w-[360px] max-w-[calc(100vw-2rem)] rounded-md bg-surface-popover border border-border shadow-overlay animate-[fade-in-up_0.2s_ease_both] flex flex-col max-h-[70vh]"
    >
      <div className="flex items-center justify-between gap-2 px-4 py-3 border-b border-border-subtle">
        <div className="flex items-center gap-2">
          <LuBell size={14} className="text-text-tertiary" />
          <span
            className="font-semibold text-text-primary text-sm"
           
          >
            Notifications
          </span>
        </div>
        <span className="text-text-tertiary text-2xs">
          {items.length} item{items.length === 1 ? '' : 's'}
        </span>
      </div>
      <div className="overflow-y-auto">
        {items.length === 0 ? (
          <div
            className="text-center py-12 text-text-tertiary text-sm"
           
          >
            All caught up.
          </div>
        ) : (
          items.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => {
                if (item.href) router.navigate({ to: item.href.to });
                closeNotifications();
              }}
              className="w-full text-left flex items-start gap-3 px-4 py-3 border-b border-border-subtle last:border-b-0 hover:bg-surface-raised cursor-pointer transition-colors"
            >
              <span
                className="mt-1 w-2 h-2 rounded-full shrink-0"
                style={{ background: toneFg[item.tone] }}
              />
              <div className="flex-1 min-w-0">
                <div
                  className="font-semibold text-text-primary text-sm"
                 
                >
                  {item.title}
                </div>
                {item.body && (
                  <div
                    className="text-text-secondary mt-0.5 text-xs"
                   
                  >
                    {item.body}
                  </div>
                )}
              </div>
              <LuChevronRight
                size={14}
                className="shrink-0 mt-1.5"
                style={{ color: toneFg[item.tone] }}
              />
              <span className="sr-only" style={{ background: toneBg[item.tone] }} />
            </button>
          ))
        )}
      </div>
    </div>
  );
}
