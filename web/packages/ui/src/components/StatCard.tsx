import type { ReactNode } from "react";

export interface StatCardProps {
  icon: ReactNode;
  value: string | number;
  label: string;
  trend?: { value: string; positive: boolean };
}

export function StatCard({ icon, value, label, trend }: StatCardProps) {
  return (
    <div className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] p-5 shadow-[var(--shadow-card)] animate-[fade-in-up_0.4s_ease_both]">
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div className="flex items-center justify-center w-10 h-10 rounded-[var(--radius-sm)] bg-[var(--color-ember-faint)] text-[var(--color-ember)]">
            {icon}
          </div>
          <div>
            <p className="text-2xl font-semibold text-[var(--color-text-primary)] leading-tight">
              {value}
            </p>
            <p className="text-xs text-[var(--color-text-secondary)] mt-0.5">{label}</p>
          </div>
        </div>
        {trend && (
          <span
            className={`text-xs font-medium px-2 py-0.5 rounded-full ${
              trend.positive
                ? "bg-[var(--color-success-faint)] text-[var(--color-success)]"
                : "bg-[var(--color-error-faint)] text-[var(--color-error)]"
            }`}
          >
            {trend.value}
          </span>
        )}
      </div>
    </div>
  );
}
