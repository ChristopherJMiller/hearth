export type StatusValue =
  // Software request lifecycle
  | "pending"
  | "approved"
  | "installing"
  | "installed"
  | "denied"
  | "failed"
  // Machine enrollment lifecycle
  | "enrolled"
  | "provisioning"
  | "active"
  | "decommissioned"
  // Deployment lifecycle
  | "canary"
  | "rolling"
  | "completed"
  | "rolled_back"
  // Build pipeline
  | "claimed"
  | "evaluating"
  | "building"
  | "pushing"
  | "deploying"
  // User-env build
  | "ready"
  | "activating"
  | "built"
  // Drift
  | "compliant"
  | "drifted"
  | "no_target"
  // Per-machine update
  | "downloading"
  | "switching"
  // Actions
  | "delivered"
  | "running"
  // Severity
  | "low"
  | "medium"
  | "high"
  | "critical"
  // Generic
  | "success"
  | "warning"
  | "error"
  | "info"
  | "idle";

export type StatusTone = "neutral" | "info" | "success" | "warning" | "danger" | "purple";

export interface StatusChipProps {
  status: StatusValue | string;
  /** Override the label text. Defaults to a humanized version of `status`. */
  label?: string;
  /** Override the automatic tone derivation. */
  tone?: StatusTone;
  /** Show the leading dot indicator. Defaults to true. */
  withDot?: boolean;
  /** Override automatic pulse (for in-flight states). */
  pulse?: boolean;
  size?: "sm" | "md";
}

const toneTokens: Record<StatusTone, { bg: string; fg: string; dot: string }> = {
  neutral: {
    bg: "var(--color-surface-raised)",
    fg: "var(--color-text-secondary)",
    dot: "var(--color-text-tertiary)",
  },
  info: {
    bg: "var(--color-info-faint)",
    fg: "var(--color-info)",
    dot: "var(--color-info)",
  },
  success: {
    bg: "var(--color-success-faint)",
    fg: "var(--color-success)",
    dot: "var(--color-success)",
  },
  warning: {
    bg: "var(--color-warning-faint)",
    fg: "var(--color-warning)",
    dot: "var(--color-warning)",
  },
  danger: {
    bg: "var(--color-error-faint)",
    fg: "var(--color-error)",
    dot: "var(--color-error)",
  },
  purple: {
    bg: "var(--color-purple-faint)",
    fg: "var(--color-purple)",
    dot: "var(--color-purple)",
  },
};

const statusConfig: Record<string, { tone: StatusTone; pulse: boolean; label?: string }> = {
  // Pending / awaiting action
  pending: { tone: "warning", pulse: true },
  // In-flight blue states
  approved: { tone: "info", pulse: true },
  installing: { tone: "info", pulse: true },
  claimed: { tone: "info", pulse: true },
  evaluating: { tone: "info", pulse: true },
  building: { tone: "info", pulse: true },
  pushing: { tone: "info", pulse: true },
  deploying: { tone: "info", pulse: true },
  canary: { tone: "info", pulse: true },
  rolling: { tone: "info", pulse: true },
  provisioning: { tone: "info", pulse: true },
  activating: { tone: "info", pulse: true },
  downloading: { tone: "info", pulse: true },
  switching: { tone: "info", pulse: true },
  delivered: { tone: "info", pulse: true },
  running: { tone: "info", pulse: true },
  // Terminal success
  installed: { tone: "success", pulse: false },
  completed: { tone: "success", pulse: false },
  active: { tone: "success", pulse: false },
  enrolled: { tone: "success", pulse: false },
  ready: { tone: "success", pulse: false },
  built: { tone: "success", pulse: false },
  compliant: { tone: "success", pulse: false },
  success: { tone: "success", pulse: false },
  // Neutral / final
  decommissioned: { tone: "neutral", pulse: false },
  no_target: { tone: "neutral", pulse: false, label: "No target" },
  rolled_back: { tone: "purple", pulse: false, label: "Rolled back" },
  idle: { tone: "neutral", pulse: false },
  // Terminal error
  denied: { tone: "danger", pulse: false },
  failed: { tone: "danger", pulse: false },
  error: { tone: "danger", pulse: false },
  drifted: { tone: "warning", pulse: false },
  warning: { tone: "warning", pulse: false },
  info: { tone: "info", pulse: false },
  // Severity
  low: { tone: "neutral", pulse: false },
  medium: { tone: "info", pulse: false },
  high: { tone: "warning", pulse: false },
  critical: { tone: "danger", pulse: false },
};

function humanize(s: string): string {
  return s
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function StatusChip({
  status,
  label,
  tone: toneOverride,
  withDot = true,
  pulse: pulseOverride,
  size = "md",
}: StatusChipProps) {
  const config = statusConfig[status] ?? { tone: "neutral" as StatusTone, pulse: false };
  const tone = toneOverride ?? config.tone;
  const shouldPulse = pulseOverride ?? config.pulse;
  const tokens = toneTokens[tone];
  const text = label ?? config.label ?? humanize(String(status));

  return (
    <span
      className="inline-flex items-center gap-2 font-medium font-sans rounded-full whitespace-nowrap"
      style={{
        fontSize: size === "sm" ? "var(--text-2xs)" : "var(--text-xs)",
        paddingTop: "var(--density-chip-y)",
        paddingBottom: "var(--density-chip-y)",
        paddingLeft: "var(--density-chip-x)",
        paddingRight: "var(--density-chip-x)",
        background: tokens.bg,
        color: tokens.fg,
        lineHeight: 1,
      }}
    >
      {withDot && (
        <span
          className={`w-1.5 h-1.5 rounded-full shrink-0 ${shouldPulse ? "animate-[pulse-dot_1.8s_ease-in-out_infinite]" : ""}`}
          style={{ background: tokens.dot }}
        />
      )}
      {text}
    </span>
  );
}
