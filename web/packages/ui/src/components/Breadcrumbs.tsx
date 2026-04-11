import { type ReactNode, Fragment } from "react";
import {
  Breadcrumb,
  BreadcrumbItem as BItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "./ui/breadcrumb";

export interface BreadcrumbItem {
  label: ReactNode;
  href?: string;
  onClick?: () => void;
}

export interface BreadcrumbsProps {
  items: BreadcrumbItem[];
  separator?: ReactNode;
}

export function Breadcrumbs({ items, separator }: BreadcrumbsProps) {
  if (items.length === 0) return null;

  return (
    <Breadcrumb>
      <BreadcrumbList className="text-text-tertiary text-xs">
        {items.map((item, i) => {
          const isLast = i === items.length - 1;
          return (
            <Fragment key={i}>
              {i > 0 && (
                <BreadcrumbSeparator className="text-border">
                  {separator}
                </BreadcrumbSeparator>
              )}
              <BItem>
                {isLast ? (
                  <BreadcrumbPage className="text-text-primary font-medium">
                    {item.label}
                  </BreadcrumbPage>
                ) : item.onClick ? (
                  <button
                    type="button"
                    onClick={item.onClick}
                    className="hover:text-text-primary transition-colors cursor-pointer"
                  >
                    {item.label}
                  </button>
                ) : item.href ? (
                  <BreadcrumbLink
                    href={item.href}
                    className="hover:text-text-primary"
                  >
                    {item.label}
                  </BreadcrumbLink>
                ) : (
                  <span>{item.label}</span>
                )}
              </BItem>
            </Fragment>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}
