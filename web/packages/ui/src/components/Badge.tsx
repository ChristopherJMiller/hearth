import type { ReactNode } from "react";

export type BadgeVariant = "flatpak" | "nix-system" | "nix-user" | "home-manager";

export interface BadgeProps {
  variant: BadgeVariant;
  children: ReactNode;
}

const variantClasses: Record<BadgeVariant, string> = {
  flatpak: "bg-[var(--color-success-faint)] text-[var(--color-success)]",
  "nix-system": "bg-[var(--color-warning-faint)] text-[var(--color-warning)]",
  "nix-user": "bg-[var(--color-info-faint)] text-[var(--color-info)]",
  "home-manager": "bg-[var(--color-purple-faint)] text-[var(--color-purple)]",
};

export function Badge({ variant, children }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center gap-1 font-mono text-[11px] font-medium uppercase tracking-wide px-2 py-0.5 rounded-md leading-snug whitespace-nowrap ${variantClasses[variant]}`}
    >
      {children}
    </span>
  );
}
