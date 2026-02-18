export interface FilterPillsProps {
  options: string[];
  active: string;
  onSelect: (value: string) => void;
}

function Pill({
  label,
  isActive,
  onClick,
}: {
  label: string;
  isActive: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center justify-center font-sans text-[13px] font-medium px-4 py-1.5 rounded-full border cursor-pointer transition-all duration-150 ease-out select-none whitespace-nowrap ${
        isActive
          ? "bg-[var(--color-ember)] border-[var(--color-ember)] text-white"
          : "bg-transparent border-[var(--color-border)] text-[var(--color-text-secondary)] hover:border-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]"
      }`}
    >
      {label}
    </button>
  );
}

export function FilterPills({ options, active, onSelect }: FilterPillsProps) {
  const allOptions = ["All", ...options.filter((o) => o !== "All")];

  return (
    <div className="flex flex-wrap gap-2 items-center" role="group">
      {allOptions.map((option) => (
        <Pill
          key={option}
          label={option}
          isActive={active === option}
          onClick={() => onSelect(option)}
        />
      ))}
    </div>
  );
}
