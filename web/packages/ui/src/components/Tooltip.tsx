import { type ReactNode, useState, useRef, useEffect, cloneElement, isValidElement } from "react";

export type TooltipSide = "top" | "bottom" | "left" | "right";

export interface TooltipProps {
  content: ReactNode;
  side?: TooltipSide;
  delay?: number;
  children: ReactNode;
  className?: string;
}

/**
 * Minimal CSS-positioned tooltip. Wraps a single child element and reveals
 * a small hint on hover/focus. No floating-ui dependency — positioning is
 * relative to the trigger and assumes normal flow.
 */
export function Tooltip({ content, side = "top", delay = 250, children, className = "" }: TooltipProps) {
  const [open, setOpen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);

  const show = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setOpen(true), delay);
  };
  const hide = () => {
    if (timer.current) clearTimeout(timer.current);
    setOpen(false);
  };

  const positionClasses: Record<TooltipSide, string> = {
    top: "bottom-[calc(100%+8px)] left-1/2 -translate-x-1/2",
    bottom: "top-[calc(100%+8px)] left-1/2 -translate-x-1/2",
    left: "right-[calc(100%+8px)] top-1/2 -translate-y-1/2",
    right: "left-[calc(100%+8px)] top-1/2 -translate-y-1/2",
  };

  const triggerProps = {
    onMouseEnter: show,
    onMouseLeave: hide,
    onFocus: show,
    onBlur: hide,
  };

  // If we got a single element child, clone it and attach handlers.
  const trigger = isValidElement(children)
    ? cloneElement(children as any, triggerProps)
    : <span {...triggerProps}>{children}</span>;

  return (
    <span className={`relative inline-flex ${className}`}>
      {trigger}
      {open && (
        <span role="tooltip"
          className={`absolute z-50 pointer-events-none whitespace-nowrap px-2 py-1 rounded-[var(--radius-sm)] bg-[var(--color-surface-popover)] border border-[var(--color-border)] text-[var(--color-text-primary)] shadow-[var(--shadow-overlay)] ${positionClasses[side]} text-2xs`}
          style={{ animation: "fade-in 0.15s ease both" }}
        >
          {content}
        </span>
      )}
    </span>
  );
}
