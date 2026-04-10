import type { ReactNode } from "react";
import { Breadcrumbs, type BreadcrumbItem } from "./Breadcrumbs";

export interface PageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  eyebrow?: ReactNode;
  breadcrumbs?: BreadcrumbItem[];
  actions?: ReactNode;
  children?: ReactNode;
}

export function PageHeader({
  title,
  description,
  eyebrow,
  breadcrumbs,
  actions,
  children,
}: PageHeaderProps) {
  return (
    <div className="flex flex-col gap-3 mb-[var(--density-section-gap)]">
      {breadcrumbs && breadcrumbs.length > 0 && <Breadcrumbs items={breadcrumbs} />}
      <div className="flex items-center justify-between gap-6 flex-wrap">
        <div className="flex flex-col gap-2 min-w-0 flex-1">
          {eyebrow && (
            <div
              className="uppercase font-semibold text-[var(--color-ember)] text-2xs tracking-wide"
             
            >
              {eyebrow}
            </div>
          )}
          <h1
            className="font-semibold text-[var(--color-text-primary)] text-2xl tracking-tight leading-tight"
           
          >
            {title}
          </h1>
          {description && (
            <p
              className="text-[var(--color-text-secondary)] max-w-[72ch] text-sm leading-body"
             
            >
              {description}
            </p>
          )}
        </div>
        {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
      </div>
      {children}
    </div>
  );
}
