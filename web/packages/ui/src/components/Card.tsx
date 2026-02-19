import { type HTMLAttributes } from "react";

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  animationDelay?: number;
}

export function Card({
  children,
  className = "",
  style,
  animationDelay,
  ...rest
}: CardProps) {
  const delayStyle = animationDelay != null
    ? { animationDelay: `${animationDelay}ms`, ...style }
    : style;

  return (
    <div
      className={`bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] p-6 shadow-[var(--shadow-card)] animate-[fade-in-up_0.4s_ease_both] transition-all duration-200 ease-out hover:-translate-y-0.5 hover:border-[var(--color-border-accent)] hover:shadow-[var(--shadow-card-hover)] ${className}`}
      style={delayStyle}
      {...rest}
    >
      {children}
    </div>
  );
}
