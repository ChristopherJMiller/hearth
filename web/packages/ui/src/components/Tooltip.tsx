import { type ReactNode } from "react";
import {
  Tooltip as ShadcnTooltip,
  TooltipContent,
  TooltipTrigger,
} from "./ui/tooltip";

export type TooltipSide = "top" | "bottom" | "left" | "right";

export interface TooltipProps {
  content: ReactNode;
  side?: TooltipSide;
  /** Delay in ms before the tooltip shows on hover. */
  delay?: number;
  children: ReactNode;
  className?: string;
}

/**
 * Tooltip wraps a single child and reveals floating hint content on hover/focus.
 * Built on Radix Tooltip via shadcn — requires a `<TooltipProvider>` ancestor
 * (mounted once near the app root in `App.tsx`).
 */
export function Tooltip({
  content,
  side = "top",
  delay = 250,
  children,
  className,
}: TooltipProps) {
  return (
    <ShadcnTooltip delayDuration={delay}>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent
        side={side}
        className={className}
        sideOffset={6}
      >
        {content}
      </TooltipContent>
    </ShadcnTooltip>
  );
}
