import { type ButtonHTMLAttributes, type ReactNode } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { Loader2 } from "lucide-react";
import { cn } from "../lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 font-sans font-semibold leading-none rounded-sm cursor-pointer select-none transition-all duration-150 ease-out disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ember whitespace-nowrap",
  {
    variants: {
      variant: {
        primary:
          "bg-ember text-white border border-transparent hover:bg-ember-dim active:scale-[0.98] shadow-[0_1px_0_rgba(0,0,0,0.2),0_0_0_0_rgba(233,69,96,0.25)] hover:shadow-[0_1px_0_rgba(0,0,0,0.2),0_6px_18px_-6px_rgba(233,69,96,0.45)]",
        outline:
          "bg-transparent text-ember border border-ember hover:bg-ember-faint",
        ghost:
          "bg-transparent text-text-secondary border border-transparent hover:text-text-primary hover:bg-surface-raised",
        subtle:
          "bg-surface-raised text-text-primary border border-border-subtle hover:bg-surface-overlay hover:border-border",
        danger:
          "bg-error-faint text-error border border-[rgba(233,69,96,0.35)] hover:bg-[rgba(233,69,96,0.18)]",
      },
      size: {
        sm: "px-3.5 py-1.5 text-xs",
        md: "px-4 py-2 text-sm",
        lg: "px-6 py-3 text-base",
      },
    },
    defaultVariants: {
      variant: "primary",
      size: "md",
    },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
  leadingIcon?: ReactNode;
  trailingIcon?: ReactNode;
}

export function Button({
  variant,
  size,
  loading = false,
  leadingIcon,
  trailingIcon,
  className,
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <Loader2 className="size-3.5 animate-spin" /> : leadingIcon}
      {children}
      {!loading && trailingIcon}
    </button>
  );
}
