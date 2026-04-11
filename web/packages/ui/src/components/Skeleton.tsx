import { type CSSProperties } from "react";

export interface SkeletonProps {
  width?: string | number;
  height?: string | number;
  radius?: "sm" | "md" | "lg" | "full";
  className?: string;
  style?: CSSProperties;
}

const radiusMap = {
  sm: "var(--radius-sm)",
  md: "var(--radius-md)",
  lg: "var(--radius-lg)",
  full: "var(--radius-full)",
};

export function Skeleton({
  width = "100%",
  height = "1rem",
  radius = "sm",
  className = "",
  style,
}: SkeletonProps) {
  return (
    <span
      aria-hidden="true"
      className={`block relative overflow-hidden bg-surface-raised ${className}`}
      style={{
        width,
        height,
        borderRadius: radiusMap[radius],
        ...style,
      }}
    >
      <span
        className="absolute inset-0"
        style={{
          background:
            "linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.05) 50%, transparent 100%)",
          animation: "shimmer 1.5s ease-in-out infinite",
        }}
      />
    </span>
  );
}

export function SkeletonText({ lines = 3, className = "" }: { lines?: number; className?: string }) {
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton key={i} height="0.875rem" width={i === lines - 1 ? "70%" : "100%"} />
      ))}
    </div>
  );
}

export function SkeletonCard({ className = "" }: { className?: string }) {
  return (
    <div
      className={`rounded-md bg-surface border border-border-subtle p-(--density-card) ${className}`}
    >
      <Skeleton height="1.25rem" width="40%" />
      <div className="mt-4">
        <SkeletonText lines={3} />
      </div>
    </div>
  );
}

export function SkeletonTable({ rows = 5, cols = 4 }: { rows?: number; cols?: number }) {
  return (
    <div className="rounded-md border border-border-subtle overflow-hidden">
      <div className="bg-surface border-b border-border-subtle py-3.5 px-5">
        <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
          {Array.from({ length: cols }).map((_, i) => (
            <Skeleton key={i} height="0.75rem" width="60%" />
          ))}
        </div>
      </div>
      {Array.from({ length: rows }).map((_, r) => (
        <div
          key={r}
          className="border-b border-border-subtle last:border-b-0 py-3.5 px-5"
        >
          <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
            {Array.from({ length: cols }).map((_, c) => (
              <Skeleton key={c} height="1rem" width={c === 0 ? "80%" : "60%"} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
