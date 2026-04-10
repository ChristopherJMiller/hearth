import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Kbd } from "./Kbd";

export interface CommandItem {
  id: string;
  label: string;
  hint?: string;
  keywords?: string;
  icon?: ReactNode;
  group?: string;
  shortcut?: string;
  onRun: () => void;
}

export interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  items: CommandItem[];
  placeholder?: string;
}

/** Simple character-subsequence fuzzy scorer. Higher is better; -1 means no match. */
function score(query: string, target: string): number {
  if (!query) return 0;
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  let qi = 0;
  let s = 0;
  let lastMatch = -1;
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] === q[qi]) {
      // Bonuses: word-start matches, consecutive matches
      if (i === 0 || t[i - 1] === " " || t[i - 1] === "-" || t[i - 1] === "/") s += 6;
      if (lastMatch === i - 1) s += 4;
      s += 2;
      lastMatch = i;
      qi++;
    }
  }
  if (qi < q.length) return -1;
  // Prefer shorter strings when equal.
  s -= Math.floor(t.length / 20);
  return s;
}

export function CommandPalette({
  open,
  onOpenChange,
  items,
  placeholder = "Search or jump to…",
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  const filtered = useMemo(() => {
    if (!query.trim()) {
      return items.slice(0, 50);
    }
    const withScores = items
      .map((item) => ({
        item,
        s: score(query, `${item.label} ${item.keywords ?? ""} ${item.hint ?? ""} ${item.group ?? ""}`),
      }))
      .filter((x) => x.s >= 0);
    withScores.sort((a, b) => b.s - a.s);
    return withScores.slice(0, 50).map((x) => x.item);
  }, [query, items]);

  useEffect(() => {
    if (active >= filtered.length) setActive(0);
  }, [filtered.length, active]);

  // Group by `group` in display order.
  const groups = useMemo(() => {
    const map = new Map<string, CommandItem[]>();
    for (const item of filtered) {
      const key = item.group ?? "Commands";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(item);
    }
    return Array.from(map.entries());
  }, [filtered]);

  if (!open) return null;

  const runAt = (idx: number) => {
    const item = filtered[idx];
    if (!item) return;
    onOpenChange(false);
    // Defer so the close animation/state flush before side-effects fire.
    setTimeout(() => item.onRun(), 0);
  };

  const handleKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => (i + 1) % Math.max(1, filtered.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => (i - 1 + filtered.length) % Math.max(1, filtered.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      runAt(active);
    }
  };

  // Map item to its index in `filtered` for keyboard highlighting.
  const indexOf = (item: CommandItem) => filtered.indexOf(item);

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-start justify-center p-4 pt-[12vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <button
        type="button"
        aria-label="Close command palette"
        onClick={() => onOpenChange(false)}
        className="absolute inset-0 bg-black/60 backdrop-blur-sm cursor-pointer animate-[fade-in_0.2s_ease_both]"
      />
      <div
        className="relative w-full max-w-[640px] bg-[var(--color-surface-popover)] rounded-[var(--radius-md)] border border-[var(--color-border)] shadow-[var(--shadow-overlay)] overflow-hidden animate-[fade-in-up_0.2s_ease_both] flex flex-col max-h-[70vh]"
      >
        <div className="flex items-center gap-3 px-4 py-3 border-b border-[var(--color-border-subtle)]">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
               className="text-[var(--color-text-tertiary)] shrink-0">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.3-4.3" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKey}
            placeholder={placeholder}
            className="flex-1 bg-transparent border-none outline-none text-[var(--color-text-primary)] placeholder:text-[var(--color-text-tertiary)] text-base"
           
          />
          <Kbd>ESC</Kbd>
        </div>
        <div className="flex-1 overflow-y-auto py-2">
          {filtered.length === 0 ? (
            <div className="text-center py-10 text-[var(--color-text-tertiary)] text-sm"
                >
              No matches. Try a different query.
            </div>
          ) : (
            groups.map(([groupName, groupItems]) => (
              <div key={groupName} className="mb-2 last:mb-0">
                <div className="px-4 py-1 uppercase font-semibold text-[var(--color-text-tertiary)] text-2xs tracking-wide"
                    >
                  {groupName}
                </div>
                {groupItems.map((item) => {
                  const idx = indexOf(item);
                  const isActive = idx === active;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onMouseMove={() => setActive(idx)}
                      onClick={() => runAt(idx)}
                      className={`w-full flex items-center gap-3 px-4 py-2.5 text-left cursor-pointer transition-colors ${
                        isActive
                          ? "bg-[var(--color-ember-faint)] text-[var(--color-text-primary)]"
                          : "text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-raised)]"
                      }`}
                    >
                      {item.icon && (
                        <span className={`shrink-0 w-5 h-5 flex items-center justify-center ${isActive ? "text-[var(--color-ember)]" : ""}`}>
                          {item.icon}
                        </span>
                      )}
                      <span className="flex-1 text-sm">
                        {item.label}
                        {item.hint && (
                          <span className="text-[var(--color-text-tertiary)] ml-2 text-xs">
                            {item.hint}
                          </span>
                        )}
                      </span>
                      {item.shortcut && <Kbd>{item.shortcut}</Kbd>}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
        <div className="flex items-center justify-between gap-4 px-4 py-2 border-t border-[var(--color-border-subtle)] bg-[var(--color-surface-sunken)] text-[var(--color-text-tertiary)] text-2xs"
            >
          <div className="flex items-center gap-3">
            <span className="flex items-center gap-1"><Kbd>↑</Kbd><Kbd>↓</Kbd> navigate</span>
            <span className="flex items-center gap-1"><Kbd>↵</Kbd> select</span>
          </div>
          <span>{filtered.length} result{filtered.length === 1 ? "" : "s"}</span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
