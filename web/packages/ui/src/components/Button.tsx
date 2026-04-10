import { type ButtonHTMLAttributes, type ReactNode } from "react";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "outline" | "ghost" | "danger" | "subtle";
  size?: "sm" | "md" | "lg";
  loading?: boolean;
  leadingIcon?: ReactNode;
  trailingIcon?: ReactNode;
}

const baseClasses =
  "inline-flex items-center justify-center gap-2 font-sans font-semibold leading-none rounded-[var(--radius-sm)] cursor-pointer select-none transition-all duration-150 ease-out disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-ember)] whitespace-nowrap";

const sizeClasses: Record<"sm" | "md" | "lg", string> = {
  sm: "px-3.5 py-2 text-xs",
  md: "px-[1.125rem] py-2.5 text-sm",
  lg: "px-6 py-3.5 text-base",
};

const variantClasses = {
  primary:
    "bg-[var(--color-ember)] text-white border border-transparent hover:bg-[var(--color-ember-dim)] active:scale-[0.98] shadow-[0_1px_0_rgba(0,0,0,0.2),0_0_0_0_rgba(233,69,96,0.25)] hover:shadow-[0_1px_0_rgba(0,0,0,0.2),0_6px_18px_-6px_rgba(233,69,96,0.45)]",
  outline:
    "bg-transparent text-[var(--color-ember)] border border-[var(--color-ember)] hover:bg-[var(--color-ember-faint)]",
  ghost:
    "bg-transparent text-[var(--color-text-secondary)] border border-transparent hover:text-[var(--color-text-primary)] hover:bg-[var(--color-surface-raised)]",
  subtle:
    "bg-[var(--color-surface-raised)] text-[var(--color-text-primary)] border border-[var(--color-border-subtle)] hover:bg-[var(--color-surface-overlay)] hover:border-[var(--color-border)]",
  danger:
    "bg-[var(--color-error-faint)] text-[var(--color-error)] border border-[rgba(233,69,96,0.35)] hover:bg-[rgba(233,69,96,0.18)]",
};

function Spinner() {
  return (
    <svg
      className="animate-spin"
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
    >
      <path d="M21 12a9 9 0 11-6.219-8.56" />
    </svg>
  );
}

export function Button({
  variant = "primary",
  size = "md",
  loading = false,
  leadingIcon,
  trailingIcon,
  className = "",
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={`${baseClasses} ${sizeClasses[size]} ${variantClasses[variant]} ${className}`}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <Spinner /> : leadingIcon}
      {children}
      {!loading && trailingIcon}
    </button>
  );
}
