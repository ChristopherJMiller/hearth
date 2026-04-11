interface FilterPillsProps {
  categories: string[];
  active: string;
  onSelect: (category: string) => void;
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
      className={`inline-flex items-center px-4 py-1.5 rounded-full border text-xs font-medium cursor-pointer transition-all duration-150 select-none whitespace-nowrap ${
        isActive
          ? "bg-ember border-ember text-white"
          : "bg-transparent border-border text-text-secondary hover:border-text-tertiary hover:text-text-primary"
      }`}
    >
      {label}
    </button>
  );
}

export function FilterPills({ categories, active, onSelect }: FilterPillsProps) {
  return (
    <div className="mt-6 flex gap-2 flex-wrap">
      <Pill label="All" isActive={active === 'all'} onClick={() => onSelect('all')} />
      {categories.map((cat) => (
        <Pill
          key={cat}
          label={cat}
          isActive={active === cat}
          onClick={() => onSelect(cat)}
        />
      ))}
    </div>
  );
}
