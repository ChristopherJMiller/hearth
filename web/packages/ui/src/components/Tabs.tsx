import { useRef, type ReactNode, type KeyboardEvent } from "react";

export interface Tab {
  id: string;
  label: ReactNode;
  icon?: ReactNode;
  count?: number;
}

export interface TabsProps {
  tabs: Tab[];
  activeId: string;
  onChange: (id: string) => void;
  orientation?: "horizontal" | "vertical";
  ariaLabel?: string;
}

/**
 * Controlled, flat Tabs. Kept hand-rolled instead of wrapping shadcn Tabs
 * because our existing call sites pass `tabs={[{id, label, icon, count}]}`
 * and `activeId`/`onChange` — the shadcn API (TabsList + TabsTrigger +
 * TabsContent children) is a different shape. Accessibility behavior here
 * matches the ARIA tablist pattern.
 */
export function Tabs({
  tabs,
  activeId,
  onChange,
  orientation = "horizontal",
  ariaLabel,
}: TabsProps) {
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});

  const handleKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const keys = orientation === "horizontal"
      ? { next: "ArrowRight", prev: "ArrowLeft" }
      : { next: "ArrowDown", prev: "ArrowUp" };
    const currentIdx = tabs.findIndex((t) => t.id === activeId);
    if (currentIdx === -1) return;
    let nextIdx = currentIdx;
    if (e.key === keys.next) nextIdx = (currentIdx + 1) % tabs.length;
    else if (e.key === keys.prev) nextIdx = (currentIdx - 1 + tabs.length) % tabs.length;
    else if (e.key === "Home") nextIdx = 0;
    else if (e.key === "End") nextIdx = tabs.length - 1;
    else return;
    e.preventDefault();
    const nextId = tabs[nextIdx].id;
    onChange(nextId);
    refs.current[nextId]?.focus();
  };

  const wrapperClass = orientation === "horizontal"
    ? "flex items-center gap-1 border-b border-border-subtle overflow-x-auto"
    : "flex flex-col gap-1 border-r border-border-subtle";

  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      aria-orientation={orientation}
      onKeyDown={handleKey}
      className={wrapperClass}
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeId;
        const activeBorder = orientation === "horizontal"
          ? (isActive ? "border-b-2 border-ember -mb-px" : "border-b-2 border-transparent -mb-px")
          : (isActive ? "border-r-2 border-ember -mr-px" : "border-r-2 border-transparent -mr-px");
        return (
          <button
            key={tab.id}
            ref={(el) => { refs.current[tab.id] = el; }}
            role="tab"
            type="button"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onChange(tab.id)}
            className={`flex items-center gap-2 px-4 py-3 text-sm font-medium cursor-pointer transition-colors duration-100 ${activeBorder} ${
              isActive
                ? "text-ember"
                : "text-text-secondary hover:text-text-primary"
            }`}
          >
            {tab.icon && <span className="w-4 h-4 shrink-0 flex items-center justify-center">{tab.icon}</span>}
            {tab.label}
            {tab.count != null && (
              <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 rounded-full bg-surface-raised text-text-secondary text-2xs">
                {tab.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
