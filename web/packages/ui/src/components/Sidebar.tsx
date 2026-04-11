import { type ReactNode, useState } from "react";

export interface SidebarItem {
  id: string;
  label: string;
  icon: ReactNode;
  href?: string;
  badge?: number;
}

export interface SidebarGroup {
  id: string;
  label: string;
  items: SidebarItem[];
}

export interface SidebarProps {
  /** Flat list of items (legacy). Use `groups` for new code. */
  items?: SidebarItem[];
  /** Grouped nav sections. Preferred over `items`. */
  groups?: SidebarGroup[];
  activeId: string;
  onNavigate: (id: string) => void;
  header?: ReactNode;
  footer?: ReactNode;
}

function ChevronIcon({ collapsed }: { collapsed: boolean }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      className={`transition-transform duration-200 ${collapsed ? "rotate-180" : ""}`}
    >
      <path d="M15 18l-6-6 6-6" />
    </svg>
  );
}

function NavButton({
  item,
  isActive,
  collapsed,
  onClick,
}: {
  item: SidebarItem;
  isActive: boolean;
  collapsed: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button"
      onClick={onClick}
      className={`flex items-center gap-3 w-full rounded-sm px-3 py-2.5 font-medium cursor-pointer transition-colors duration-100 ${
        isActive
          ? "bg-ember-faint text-ember border border-border-accent shadow-[inset_0_0_0_1px_rgba(233,69,96,0.15)]"
          : "text-text-secondary hover:text-text-primary hover:bg-surface-raised border border-transparent"
      } text-sm`}
     
      title={collapsed ? item.label : undefined}
    >
      <span className="flex items-center justify-center w-5 h-5 shrink-0">
        {item.icon}
      </span>
      {!collapsed && (
        <>
          <span className="truncate flex-1 text-left">{item.label}</span>
          {item.badge != null && item.badge > 0 && (
            <span
              className="flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-ember text-white font-semibold text-2xs"
             
            >
              {item.badge}
            </span>
          )}
        </>
      )}
    </button>
  );
}

export function Sidebar({
  items,
  groups,
  activeId,
  onNavigate,
  header,
  footer,
}: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <aside
      className={`flex flex-col h-full bg-surface border-r border-border-subtle transition-all duration-200 ${
        collapsed ? "w-16" : "w-[260px]"
      }`}
    >
      <div className="flex items-center justify-between px-4 py-4 border-b border-border-subtle">
        {!collapsed && (
          <div className="flex items-center gap-2 overflow-hidden">{header}</div>
        )}
        <button
          type="button"
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-center justify-center w-8 h-8 rounded-sm text-text-tertiary hover:text-text-primary hover:bg-surface-raised transition-colors duration-100 cursor-pointer shrink-0"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <ChevronIcon collapsed={collapsed} />
        </button>
      </div>

      <nav className="flex-1 py-3 px-2 overflow-y-auto">
        {groups ? (
          <div className="flex flex-col gap-5">
            {groups.map((group) => (
              <div key={group.id} className="flex flex-col gap-1">
                {!collapsed && (
                  <div
                    className="px-3 pb-1 font-semibold uppercase text-text-tertiary text-2xs tracking-wide"
                   
                  >
                    {group.label}
                  </div>
                )}
                <div className="flex flex-col gap-0.5">
                  {group.items.map((item) => (
                    <NavButton
                      key={item.id}
                      item={item}
                      isActive={item.id === activeId}
                      collapsed={collapsed}
                      onClick={() => onNavigate(item.id)}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-1">
            {(items ?? []).map((item) => (
              <NavButton
                key={item.id}
                item={item}
                isActive={item.id === activeId}
                collapsed={collapsed}
                onClick={() => onNavigate(item.id)}
              />
            ))}
          </div>
        )}
      </nav>

      {footer && !collapsed && (
        <div className="border-t border-border-subtle">
          {footer}
        </div>
      )}
    </aside>
  );
}
