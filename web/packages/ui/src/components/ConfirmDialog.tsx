import * as Dialog from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import { Button } from "./Button";

export interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: "danger" | "default";
  onConfirm: () => void;
  children?: ReactNode;
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  variant = "default",
  onConfirm,
  children,
}: ConfirmDialogProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      {children && <Dialog.Trigger asChild>{children}</Dialog.Trigger>}
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-[100] animate-[fade-in_0.15s_ease]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-[101] bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-lg)] shadow-[var(--shadow-overlay)] p-6 w-full max-w-md animate-[fade-in-up_0.2s_ease]">
          <Dialog.Title className="text-base font-semibold text-[var(--color-text-primary)]">
            {title}
          </Dialog.Title>
          <Dialog.Description className="text-sm text-[var(--color-text-secondary)] mt-2">
            {description}
          </Dialog.Description>
          <div className="flex items-center justify-end gap-2 mt-6">
            <Dialog.Close asChild>
              <Button variant="ghost" size="sm">
                {cancelLabel}
              </Button>
            </Dialog.Close>
            <Button
              variant={variant === "danger" ? "danger" : "primary"}
              size="sm"
              onClick={() => {
                onConfirm();
                onOpenChange(false);
              }}
            >
              {confirmLabel}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
