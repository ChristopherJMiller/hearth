export type StatusValue = "pending" | "approved" | "installing" | "installed" | "denied" | "failed";

export interface StatusChipProps {
  status: StatusValue;
}

interface StatusConfig {
  label: string;
  dotClass: string;
  chipClass: string;
  pulse: boolean;
}

const statusMap: Record<StatusValue, StatusConfig> = {
  pending: {
    label: "Pending",
    dotClass: "bg-[var(--color-warning)]",
    chipClass: "bg-[var(--color-warning-faint)] text-[var(--color-warning)]",
    pulse: true,
  },
  approved: {
    label: "Approved",
    dotClass: "bg-[var(--color-info)]",
    chipClass: "bg-[var(--color-info-faint)] text-[var(--color-info)]",
    pulse: true,
  },
  installing: {
    label: "Installing",
    dotClass: "bg-[var(--color-info)]",
    chipClass: "bg-[var(--color-info-faint)] text-[var(--color-info)]",
    pulse: true,
  },
  installed: {
    label: "Installed",
    dotClass: "bg-[var(--color-success)]",
    chipClass: "bg-[var(--color-success-faint)] text-[var(--color-success)]",
    pulse: false,
  },
  denied: {
    label: "Denied",
    dotClass: "bg-[var(--color-error)]",
    chipClass: "bg-[var(--color-error-faint)] text-[var(--color-error)]",
    pulse: false,
  },
  failed: {
    label: "Failed",
    dotClass: "bg-[var(--color-error)]",
    chipClass: "bg-[var(--color-error-faint)] text-[var(--color-error)]",
    pulse: false,
  },
};

export function StatusChip({ status }: StatusChipProps) {
  const config = statusMap[status];

  return (
    <span
      className={`inline-flex items-center gap-1.5 text-xs font-medium font-sans px-2.5 py-1 rounded-full leading-snug whitespace-nowrap ${config.chipClass}`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full shrink-0 ${config.dotClass} ${config.pulse ? "animate-[pulse-dot_1.8s_ease-in-out_infinite]" : ""}`}
      />
      {config.label}
    </span>
  );
}
