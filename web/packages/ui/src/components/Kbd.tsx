import { type ReactNode } from "react";

export interface KbdProps {
  children: ReactNode;
  className?: string;
}

export function Kbd({ children, className = "" }: KbdProps) {
  return (
    <kbd className={`inline-flex items-center justify-center min-w-[1.5rem] h-6 px-1.5 rounded-[6px] font-mono font-medium bg-surface-sunken border border-border text-text-secondary ${className} text-2xs`}
     
    >
      {children}
    </kbd>
  );
}
