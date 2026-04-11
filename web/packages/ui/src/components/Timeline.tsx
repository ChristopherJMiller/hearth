import { type ReactNode } from "react";

export type TimelineTone = "default" | "ember" | "success" | "warning" | "danger" | "info";

export interface TimelineEvent {
  id: string;
  time: ReactNode;
  title: ReactNode;
  body?: ReactNode;
  icon?: ReactNode;
  tone?: TimelineTone;
  actor?: ReactNode;
  onClick?: () => void;
}

export interface TimelineProps {
  events: TimelineEvent[];
  emptyLabel?: string;
}

const toneColor: Record<TimelineTone, string> = {
  default: "var(--color-text-secondary)",
  ember: "var(--color-ember)",
  success: "var(--color-success)",
  warning: "var(--color-warning)",
  danger: "var(--color-error)",
  info: "var(--color-info)",
};

const toneBg: Record<TimelineTone, string> = {
  default: "var(--color-surface-raised)",
  ember: "var(--color-ember-faint)",
  success: "var(--color-success-faint)",
  warning: "var(--color-warning-faint)",
  danger: "var(--color-error-faint)",
  info: "var(--color-info-faint)",
};

export function Timeline({ events, emptyLabel = "No activity yet" }: TimelineProps) {
  if (events.length === 0) {
    return (
      <div
        className="text-center py-8 text-text-tertiary text-sm"
       
      >
        {emptyLabel}
      </div>
    );
  }
  return (
    <ol className="relative flex flex-col">
      {events.map((event, i) => {
        const tone = event.tone ?? "default";
        const isLast = i === events.length - 1;
        const clickable = event.onClick !== undefined;
        return (
          <li key={event.id} className="flex gap-4 relative pb-5 last:pb-0">
            {!isLast && (
              <span
                className="absolute left-[15px] top-8 bottom-0 w-px bg-border-subtle"
                aria-hidden="true"
              />
            )}
            <div
              className="shrink-0 z-10 w-8 h-8 rounded-full flex items-center justify-center border"
              style={{
                background: toneBg[tone],
                borderColor: toneColor[tone],
                color: toneColor[tone],
              }}
            >
              {event.icon ?? <span className="w-2 h-2 rounded-full" style={{ background: toneColor[tone] }} />}
            </div>
            <button
              type={clickable ? "button" : undefined}
              onClick={event.onClick}
              className={`flex-1 flex flex-col gap-1 min-w-0 text-left ${
                clickable
                  ? "rounded-sm -mx-3 px-3 py-1.5 cursor-pointer hover:bg-surface-raised transition-colors"
                  : ""
              }`}
              disabled={!clickable}
            >
              <div className="flex items-baseline justify-between gap-3">
                <div
                  className="font-medium text-text-primary text-sm"
                 
                >
                  {event.title}
                </div>
                <div
                  className="text-text-tertiary shrink-0 tabular-nums text-2xs"
                 
                >
                  {event.time}
                </div>
              </div>
              {event.body && (
                <div
                  className="text-text-secondary text-xs"
                 
                >
                  {event.body}
                </div>
              )}
              {event.actor && (
                <div
                  className="text-text-tertiary text-2xs"
                 
                >
                  by {event.actor}
                </div>
              )}
            </button>
          </li>
        );
      })}
    </ol>
  );
}
