import { useEffect, useState } from "react";
import type { ToastItem } from "../hooks/useToast";

export interface ToastContainerProps {
  toasts: ToastItem[];
}

const typeClasses: Record<ToastItem["type"], string> = {
  success: "border-l-[var(--color-success)]",
  error: "border-l-[var(--color-error)]",
  info: "border-l-[var(--color-info)]",
};

function ToastItemView({ toast }: { toast: ToastItem }) {
  const [exiting, setExiting] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setExiting(true), 3700);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div
      className={`pointer-events-auto bg-[var(--color-surface)] border border-[var(--color-border-subtle)] border-l-[3px] rounded-[var(--radius-sm)] px-4 py-3 font-sans text-sm text-[var(--color-text-primary)] max-w-[360px] shadow-[var(--shadow-overlay)] ${typeClasses[toast.type]} ${
        exiting
          ? "animate-[slide-out-right_0.3s_ease_both]"
          : "animate-[slide-in-right_0.3s_ease_both]"
      }`}
    >
      {toast.message}
    </div>
  );
}

export function ToastContainer({ toasts }: ToastContainerProps) {
  return (
    <div className="fixed bottom-6 right-6 z-[9999] flex flex-col gap-2 pointer-events-none">
      {toasts.map((t) => (
        <ToastItemView key={t.id} toast={t} />
      ))}
    </div>
  );
}
