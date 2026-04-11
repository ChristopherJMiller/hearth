import type { ReactNode } from "react";

export type BadgeVariant = "flatpak" | "nix-system" | "nix-user" | "home-manager";

export interface BadgeProps {
  variant: BadgeVariant;
  children: ReactNode;
}

const variantClasses: Record<BadgeVariant, string> = {
  flatpak: "bg-success-faint text-success",
  "nix-system": "bg-warning-faint text-warning",
  "nix-user": "bg-info-faint text-info",
  "home-manager": "bg-purple-faint text-purple",
};

export function Badge({ variant, children }: BadgeProps) {
  return (
    <span className={`inline-flex items-center gap-1 font-mono font-medium uppercase px-2 py-0.5 rounded-md leading-snug whitespace-nowrap ${variantClasses[variant]} text-2xs tracking-wide`}
     
    >
      {children}
    </span>
  );
}
