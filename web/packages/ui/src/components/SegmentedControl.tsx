import { type ReactNode } from "react";

export interface SegmentOption<T extends string = string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
}

export interface SegmentedControlProps<T extends string = string> {
  options: SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
  ariaLabel?: string;
}

export function SegmentedControl<T extends string = string>({
  options,
  value,
  onChange,
  size = "md",
  className = "",
  ariaLabel,
}: SegmentedControlProps<T>) {
  const padding = size === "sm" ? "px-2.5 py-1.5 text-xs" : "px-3.5 py-2 text-sm";
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={`inline-flex items-center gap-1 p-1 rounded-sm bg-surface-sunken border border-border-subtle ${className}`}
    >
      {options.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(opt.value)}
            className={`flex items-center gap-1.5 ${padding} rounded-[6px] font-medium cursor-pointer transition-colors ${
              active
                ? "bg-surface-raised text-text-primary shadow-[0_1px_0_rgba(255,255,255,0.04)]"
                : "text-text-tertiary hover:text-text-secondary"
            }`}
          >
            {opt.icon && <span className="shrink-0">{opt.icon}</span>}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
