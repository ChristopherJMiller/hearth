import { type ReactNode } from "react";

export type PageSize = "narrow" | "default" | "wide" | "full";

export interface PageContainerProps {
  size?: PageSize;
  children: ReactNode;
  className?: string;
}

const sizeMap: Record<PageSize, string> = {
  narrow: "max-w-[840px]",
  default: "max-w-[1400px]",
  wide: "max-w-[1720px]",
  full: "max-w-none",
};

/**
 * Centers page content at a chosen max-width and provides a uniform
 * vertical rhythm between its direct children via `gap-section` (40px).
 *
 * This is the single source of truth for spacing between top-level page
 * blocks — `PageHeader`, metric grids, content sections, etc. all inherit
 * the same gap without needing their own `mb-section` / `mb-...` classes.
 *
 * Horizontal/vertical page padding is owned by the shell's <main> element,
 * so this component does not add its own padding.
 */
export function PageContainer({ size = "default", children, className = "" }: PageContainerProps) {
  return (
    <div className={`mx-auto w-full flex flex-col gap-card-gap ${sizeMap[size]} ${className}`}>
      {children}
    </div>
  );
}
