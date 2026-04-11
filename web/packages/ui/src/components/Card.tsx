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
      className={`bg-surface border border-border-subtle rounded-md p-6 shadow-card animate-[fade-in-up_0.4s_ease_both] transition-all duration-200 ease-out hover:border-border-accent hover:shadow-card-hover ${className}`}
      style={delayStyle}
      {...rest}
    >
      {children}
    </div>
  );
}
