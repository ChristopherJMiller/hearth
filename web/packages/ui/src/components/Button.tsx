import { type ButtonHTMLAttributes } from "react";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "outline" | "ghost";
  size?: "sm" | "md";
}

const baseClasses =
  "inline-flex items-center justify-center gap-2 font-sans font-semibold leading-none rounded-[var(--radius-sm)] cursor-pointer select-none transition-all duration-150 ease-out disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-ember)]";

const sizeClasses = {
  sm: "text-[13px] px-4 py-2.5",
  md: "text-sm px-5 py-3",
};

const variantClasses = {
  primary:
    "bg-[var(--color-ember)] text-white border border-transparent hover:bg-[var(--color-ember-dim)] active:bg-[#a83345] active:scale-[0.97]",
  outline:
    "bg-transparent text-[var(--color-ember)] border border-[var(--color-ember)] hover:bg-[var(--color-ember-faint)] active:bg-[var(--color-ember-glow)]",
  ghost:
    "bg-transparent text-[var(--color-text-secondary)] border border-transparent hover:text-[var(--color-text-primary)] hover:bg-[var(--color-surface-raised)]",
};

export function Button({
  variant = "primary",
  size = "md",
  className = "",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={`${baseClasses} ${sizeClasses[size]} ${variantClasses[variant]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
