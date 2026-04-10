import { type ReactNode, useEffect } from "react";
import { createPortal } from "react-dom";
import { lockBodyScroll } from "../lib/scrollLock";

export type ModalSize = "sm" | "md" | "lg";

export interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  size?: ModalSize;
  title?: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  dismissable?: boolean;
}

const sizeWidth: Record<ModalSize, string> = {
  sm: "420px",
  md: "560px",
  lg: "720px",
};

export function Modal({
  open,
  onOpenChange,
  size = "md",
  title,
  description,
  footer,
  children,
  dismissable = true,
}: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const release = lockBodyScroll();
    const onKey = (e: KeyboardEvent) => {
      if (dismissable && e.key === "Escape") onOpenChange(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      release();
      window.removeEventListener("keydown", onKey);
    };
  }, [open, onOpenChange, dismissable]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4" role="dialog" aria-modal="true">
      <button
        type="button"
        aria-label="Close"
        onClick={() => dismissable && onOpenChange(false)}
        className="absolute inset-0 bg-black/60 backdrop-blur-sm cursor-pointer animate-[fade-in_0.2s_ease_both]"
      />
      <div
        className="relative bg-[var(--color-surface)] rounded-[var(--radius-lg)] border border-[var(--color-border-subtle)] shadow-[var(--shadow-overlay)] flex flex-col animate-[fade-in-up_0.25s_ease_both] max-h-[90vh]"
        style={{ width: sizeWidth[size] }}
      >
        {(title || description) && (
          <div className="flex flex-col gap-1 border-b border-[var(--color-border-subtle)] p-6">
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
        )}

        <div className="flex-1 overflow-y-auto p-6">{children}</div>

        {footer && (
          <div className="flex items-center justify-end gap-2 border-t border-[var(--color-border-subtle)] bg-[var(--color-surface-sunken)] rounded-b-[var(--radius-lg)] p-6">
            {footer}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
