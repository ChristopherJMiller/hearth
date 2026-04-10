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
 * Centers page content at a chosen max-width. Horizontal/vertical page
 * padding is owned by the shell's <main> element — this component does
 * not add its own padding, so it can nest inside the padded main area
 * without doubling up.
 */
export function PageContainer({ size = "default", children, className = "" }: PageContainerProps) {
  return <div className={`mx-auto w-full ${sizeMap[size]} ${className}`}>{children}</div>;
}
