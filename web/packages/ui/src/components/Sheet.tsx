import { type ReactNode } from "react";
import {
  Sheet as ShadcnSheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "./ui/sheet";

export type SheetSide = "right" | "left";
export type SheetSize = "sm" | "md" | "lg" | "xl";

export interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  side?: SheetSide;
  size?: SheetSize;
  title?: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
}

const sizeStyle: Record<SheetSize, string> = {
  sm: "sm:max-w-[400px]",
  md: "sm:max-w-[560px]",
  lg: "sm:max-w-[720px]",
  xl: "sm:max-w-[960px]",
};

export function Sheet({
  open,
  onOpenChange,
  side = "right",
  size = "md",
  title,
  description,
  footer,
  children,
}: SheetProps) {
  return (
    <ShadcnSheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side={side}
        className={`${sizeStyle[size]} w-[92vw] bg-surface border-border-subtle gap-0 p-0`}
      >
        {(title || description) && (
          <SheetHeader className="p-6 border-b border-border-subtle">
            {title && (
              <SheetTitle className="text-xl text-text-primary">
                {title}
              </SheetTitle>
            )}
            {description && (
              <SheetDescription className="text-text-secondary">
                {description}
              </SheetDescription>
            )}
          </SheetHeader>
        )}
        <div className="flex-1 overflow-y-auto p-6">{children}</div>
        {footer && (
          <SheetFooter className="border-t border-border-subtle bg-surface-sunken p-6 mt-0">
            {footer}
          </SheetFooter>
        )}
      </SheetContent>
    </ShadcnSheet>
  );
}
