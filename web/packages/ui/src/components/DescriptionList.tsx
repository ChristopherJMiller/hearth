import { type ReactNode } from "react";

export interface DescriptionListItem {
  label: ReactNode;
  value: ReactNode;
  icon?: ReactNode;
  mono?: boolean;
  span?: 1 | 2 | 3;
}

export interface DescriptionListProps {
  items: DescriptionListItem[];
  columns?: 1 | 2 | 3;
  className?: string;
}

export function DescriptionList({ items, columns = 2, className = "" }: DescriptionListProps) {
  const gridCols =
    columns === 1 ? "grid-cols-1" : columns === 2 ? "grid-cols-1 md:grid-cols-2" : "grid-cols-1 md:grid-cols-2 lg:grid-cols-3";

  return (
    <dl className={`grid gap-x-6 gap-y-5 ${gridCols} ${className}`}>
      {items.map((item, i) => (
        <div
          key={i}
          className={`flex flex-col gap-1.5 min-w-0 ${item.span === 2 ? "md:col-span-2" : item.span === 3 ? "md:col-span-3" : ""}`}
        >
          <dt className="flex items-center gap-1.5 text-[var(--color-text-tertiary)] uppercase font-semibold text-2xs tracking-wide"
             >
            {item.icon && <span className="shrink-0">{item.icon}</span>}
            {item.label}
          </dt>
          <dd
            className={`text-[var(--color-text-primary)] break-words ${item.mono ? "font-mono text-xs" : "text-sm"}`}
          >
            {item.value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
