import { type ReactNode, useEffect } from "react";
import { createPortal } from "react-dom";
import { lockBodyScroll } from "../lib/scrollLock";

export type SheetSide = "right" | "left";
export type SheetSize = "sm" | "md" | "lg" | "xl";

export interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  side?: SheetSide;
  size?: SheetSize;
  title?: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
}

const sizeWidth: Record<SheetSize, string> = {
  sm: "400px",
  md: "560px",
  lg: "720px",
  xl: "960px",
};

export function Sheet({
  open,
  onOpenChange,
  side = "right",
  size = "md",
  title,
  description,
  footer,
  children,
}: SheetProps) {
  useEffect(() => {
    if (!open) return;
    const release = lockBodyScroll();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onOpenChange(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      release();
      window.removeEventListener("keydown", onKey);
    };
  }, [open, onOpenChange]);

  if (!open) return null;

  const sideClass = side === "right" ? "right-0" : "left-0";
  const animClass =
    side === "right"
      ? "animate-[slide-in-right_0.3s_ease_both]"
      : "animate-[fade-in_0.3s_ease_both]";

  return createPortal(
    <div className="fixed inset-0 z-50 flex" role="dialog" aria-modal="true">
      <button
        type="button"
        aria-label="Close sheet"
        onClick={() => onOpenChange(false)}
        className="absolute inset-0 bg-black/50 cursor-pointer animate-[fade-in_0.2s_ease_both]"
      />
      <div
        className={`absolute top-0 bottom-0 ${sideClass} ${animClass} bg-[var(--color-surface)] border-l border-[var(--color-border-subtle)] shadow-[var(--shadow-overlay)] flex flex-col`}
        style={{ width: sizeWidth[size], maxWidth: "92vw" }}
      >
        {(title || description) && (
          <div className="flex items-start justify-between gap-4 border-b border-[var(--color-border-subtle)] p-6">
            <div className="flex flex-col gap-1 min-w-0">
              {title && (
                <h2
                  className="font-semibold text-[var(--color-text-primary)] text-xl"
                 
                >
                  {title}
                </h2>
              )}
              {description && (
                <p
                  className="text-[var(--color-text-secondary)] text-sm"
                 
                >
                  {description}
                </p>
              )}
            </div>
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="shrink-0 w-8 h-8 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer"
              aria-label="Close"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}

        <div className="flex-1 overflow-y-auto p-6">{children}</div>

        {footer && (
          <div className="border-t border-[var(--color-border-subtle)] bg-[var(--color-surface-sunken)] p-6">
            {footer}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
