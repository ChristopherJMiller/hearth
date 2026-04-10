import { type ReactNode } from "react";

export type MetricTone = "default" | "ember" | "success" | "warning" | "danger" | "info";

export interface MetricDelta {
  value: number;
  direction: "up" | "down" | "flat";
  period?: string;
}

export interface MetricTileProps {
  label: string;
  value: ReactNode;
  delta?: MetricDelta;
  icon?: ReactNode;
  sparkline?: number[];
  tone?: MetricTone;
  onClick?: () => void;
  className?: string;
  sublabel?: ReactNode;
}

const toneColor: Record<MetricTone, string> = {
  default: "var(--color-text-primary)",
  ember: "var(--color-ember)",
  success: "var(--color-success)",
  warning: "var(--color-warning)",
  danger: "var(--color-error)",
  info: "var(--color-info)",
};

const toneGlow: Record<MetricTone, string> = {
  default: "transparent",
  ember: "var(--color-ember-faint)",
  success: "var(--color-success-faint)",
  warning: "var(--color-warning-faint)",
  danger: "var(--color-error-faint)",
  info: "var(--color-info-faint)",
};

function Sparkline({ data, color }: { data: number[]; color: string }) {
  if (data.length < 2) return null;
  const w = 120;
  const h = 32;
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;
  const step = w / (data.length - 1);
  const points = data
    .map((v, i) => `${i * step},${h - ((v - min) / range) * h}`)
    .join(" ");
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} className="overflow-visible">
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function MetricTile({
  label,
  value,
  delta,
  icon,
  sparkline,
  tone = "default",
  onClick,
  className = "",
  sublabel,
}: MetricTileProps) {
  const color = toneColor[tone];
  // `self-start` keeps the tile from being stretched to its grid row's height
  // — siblings with more content (e.g. a `sublabel`) used to drag every other
  // tile's height along with them, which read visually as "no padding".
  const wrapperClass = `group self-start relative flex flex-col gap-3 text-left rounded-[var(--radius-md)] bg-[var(--color-surface)] border border-[var(--color-border-subtle)] overflow-hidden p-8 transition-all duration-200 ${
    onClick ? "cursor-pointer hover:border-[var(--color-border-accent)] hover:bg-[var(--color-surface-raised)]" : ""
  } ${className}`;
  const inner = (
    <>
      {tone !== "default" && (
        <span
          className="absolute inset-x-0 top-0 h-[3px]"
          style={{ background: color, boxShadow: `0 0 16px ${toneGlow[tone]}` }}
        />
      )}

      <div className="flex items-center justify-between gap-3">
        <div className="flex flex-col gap-1 min-w-0">
          <span className="uppercase font-semibold text-[var(--color-text-tertiary)] text-2xs tracking-wide">
            {label}
          </span>
        </div>
        {icon && (
          <span
            className="shrink-0 w-10 h-10 rounded-[var(--radius-sm)] flex items-center justify-center"
            style={{ background: toneGlow[tone], color }}
          >
            {icon}
          </span>
        )}
      </div>

      <div className="flex items-end justify-between gap-3">
        <div className="font-semibold leading-none text-[var(--color-text-primary)] tabular-nums text-3xl tracking-tight">
          {value}
        </div>
        {sparkline && sparkline.length > 1 && <Sparkline data={sparkline} color={color} />}
      </div>

      {(sublabel || delta) && (
        <div className="flex items-center gap-2 text-[var(--color-text-secondary)] text-xs">
          {delta && (
            <span
              className="inline-flex items-center gap-1 font-medium"
              style={{
                color:
                  delta.direction === "up"
                    ? "var(--color-success)"
                    : delta.direction === "down"
                      ? "var(--color-error)"
                      : "var(--color-text-tertiary)",
              }}
            >
              {delta.direction === "up" ? "▲" : delta.direction === "down" ? "▼" : "—"}
              {Math.abs(delta.value)}
              {delta.period && <span className="text-[var(--color-text-tertiary)] ml-0.5">/{delta.period}</span>}
            </span>
          )}
          {sublabel}
        </div>
      )}
    </>
  );

  if (onClick) {
    return (
      <button type="button" onClick={onClick} className={wrapperClass}>
        {inner}
      </button>
    );
  }
  return <div className={wrapperClass}>{inner}</div>;
}
