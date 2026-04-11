export interface ProgressBarProps {
  value: number;
  max?: number;
  label?: string;
  variant?: "default" | "success" | "error";
  size?: "sm" | "md";
}

const variantColors = {
  default: "bg-ember",
  success: "bg-success",
  error: "bg-error",
};

export function ProgressBar({
  value,
  max = 100,
  label,
  variant = "default",
  size = "md",
}: ProgressBarProps) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));

  return (
    <div className="w-full">
      {label && (
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-xs text-text-secondary">{label}</span>
          <span className="text-xs font-mono text-text-tertiary">
            {Math.round(pct)}%
          </span>
        </div>
      )}
      <div
        className={`w-full bg-surface-raised rounded-full overflow-hidden ${
          size === "sm" ? "h-1.5" : "h-2.5"
        }`}
      >
        <div
          className={`h-full rounded-full transition-all duration-300 ease-out ${variantColors[variant]}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
