import { type ReactNode, Fragment } from "react";

export interface BreadcrumbItem {
  label: ReactNode;
  href?: string;
  onClick?: () => void;
}

export interface BreadcrumbsProps {
  items: BreadcrumbItem[];
  separator?: ReactNode;
}

function ChevronSeparator() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M9 6l6 6-6 6" />
    </svg>
  );
}

export function Breadcrumbs({ items, separator }: BreadcrumbsProps) {
  if (items.length === 0) return null;
  const sep = separator ?? <ChevronSeparator />;

  return (
    <nav
      aria-label="Breadcrumb"
      className="flex items-center gap-2 text-[var(--color-text-tertiary)] text-xs"
     
    >
      {items.map((item, i) => {
        const isLast = i === items.length - 1;
        const content = item.onClick || item.href
          ? (
              <button
                type="button"
                onClick={item.onClick}
                className="hover:text-[var(--color-text-primary)] transition-colors cursor-pointer"
              >
                {item.label}
              </button>
            )
          : (
              <span className={isLast ? "text-[var(--color-text-primary)] font-medium" : ""}>
                {item.label}
              </span>
            );
        return (
          <Fragment key={i}>
            {i > 0 && <span className="text-[var(--color-border)] shrink-0">{sep}</span>}
            {content}
          </Fragment>
        );
      })}
    </nav>
  );
}
