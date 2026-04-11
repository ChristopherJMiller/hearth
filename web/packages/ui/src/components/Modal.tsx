import { type ReactNode } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

export type ModalSize = "sm" | "md" | "lg";

export interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  size?: ModalSize;
  title?: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  dismissable?: boolean;
}

const sizeWidth: Record<ModalSize, string> = {
  sm: "sm:max-w-[420px]",
  md: "sm:max-w-[560px]",
  lg: "sm:max-w-[720px]",
};

/**
 * Hearth Modal — wraps shadcn Dialog with a size prop and the Hearth surface
 * palette. Scroll lock, focus trap, and escape handling are delegated to Radix.
 */
export function Modal({
  open,
  onOpenChange,
  size = "md",
  title,
  description,
  footer,
  children,
  dismissable = true,
}: ModalProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!dismissable && !next) return;
        onOpenChange(next);
      }}
    >
      <DialogContent
        showCloseButton={dismissable}
        className={`${sizeWidth[size]} bg-surface border-border-subtle p-0 gap-0 max-h-[90vh]`}
        onEscapeKeyDown={(e) => {
          if (!dismissable) e.preventDefault();
        }}
        onInteractOutside={(e) => {
          if (!dismissable) e.preventDefault();
        }}
      >
        {(title || description) && (
          <DialogHeader className="p-6 border-b border-border-subtle">
            {title && (
              <DialogTitle className="text-xl text-text-primary">
                {title}
              </DialogTitle>
            )}
            {description && (
              <DialogDescription className="text-text-secondary">
                {description}
              </DialogDescription>
            )}
          </DialogHeader>
        )}
        <div className="flex-1 overflow-y-auto p-6">{children}</div>
        {footer && (
          <DialogFooter className="border-t border-border-subtle bg-surface-sunken p-6 rounded-b-lg">
            {footer}
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
