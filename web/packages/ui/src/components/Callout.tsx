import { type ReactNode } from "react";

export type CalloutVariant = "info" | "success" | "warning" | "danger" | "neutral";

export interface CalloutProps {
  variant?: CalloutVariant;
  title?: ReactNode;
  icon?: ReactNode;
  action?: ReactNode;
  children?: ReactNode;
  className?: string;
}

const variantStyle: Record<
  CalloutVariant,
  { bg: string; border: string; accent: string; icon: string }
> = {
  info: {
    bg: "var(--color-info-faint)",
    border: "rgba(100, 149, 237, 0.35)",
    accent: "var(--color-info)",
    icon: "i",
  },
  success: {
    bg: "var(--color-success-faint)",
    border: "rgba(78, 204, 163, 0.35)",
    accent: "var(--color-success)",
    icon: "✓",
  },
  warning: {
    bg: "var(--color-warning-faint)",
    border: "rgba(240, 165, 0, 0.35)",
    accent: "var(--color-warning)",
    icon: "!",
  },
  danger: {
    bg: "var(--color-error-faint)",
    border: "rgba(233, 69, 96, 0.35)",
    accent: "var(--color-error)",
    icon: "!",
  },
  neutral: {
    bg: "var(--color-surface-raised)",
    border: "var(--color-border)",
    accent: "var(--color-text-secondary)",
    icon: "i",
  },
};

export function Callout({
  variant = "info",
  title,
  icon,
  action,
  children,
  className = "",
}: CalloutProps) {
  const s = variantStyle[variant];
  return (
    <div
      className={`flex items-start gap-3 rounded-[var(--radius-md)] border ${className}`}
      style={{
        background: s.bg,
        borderColor: s.border,
        padding: "var(--density-card)",
      }}
      role={variant === "danger" || variant === "warning" ? "alert" : undefined}
    >
      <div
        className="shrink-0 w-7 h-7 rounded-full flex items-center justify-center font-semibold text-sm"
        style={{ background: "rgba(0,0,0,0.2)" }}
      >
        {icon ?? s.icon}
      </div>
      <div className="flex-1 min-w-0 flex flex-col gap-1">
        {title && (
          <div
            className="font-semibold text-[var(--color-text-primary)] text-sm"
           
          >
            {title}
          </div>
        )}
        {children && (
          <div
            className="text-[var(--color-text-secondary)] text-sm leading-body"
           
          >
            {children}
          </div>
        )}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
