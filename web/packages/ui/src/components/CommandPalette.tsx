import { type ReactNode, useMemo } from "react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "./ui/command";
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

export function CommandPalette({
  open,
  onOpenChange,
  items,
  placeholder = "Search or jump to…",
}: CommandPaletteProps) {
  // Group items for stable display order.
  const groups = useMemo(() => {
    const map = new Map<string, CommandItem[]>();
    for (const item of items) {
      const key = item.group ?? "Commands";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(item);
    }
    return Array.from(map.entries());
  }, [items]);

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Command palette"
      description="Search commands and routes"
      className="bg-surface-popover border-border"
    >
      <CommandInput placeholder={placeholder} />
      <CommandList className="max-h-[60vh]">
        <CommandEmpty className="text-text-tertiary">
          No matches. Try a different query.
        </CommandEmpty>
        {groups.map(([groupName, groupItems]) => (
          <CommandGroup
            key={groupName}
            heading={groupName}
            className="text-text-primary"
          >
            {groupItems.map((item) => (
              <CommandItem
                key={item.id}
                value={`${item.label} ${item.keywords ?? ""} ${item.hint ?? ""} ${item.group ?? ""}`}
                onSelect={() => {
                  onOpenChange(false);
                  // Defer so the close state flushes before side effects.
                  setTimeout(() => item.onRun(), 0);
                }}
                className="data-[selected=true]:bg-ember-faint data-[selected=true]:text-text-primary"
              >
                {item.icon && (
                  <span className="shrink-0 w-5 h-5 flex items-center justify-center">
                    {item.icon}
                  </span>
                )}
                <span className="flex-1 text-sm">
                  {item.label}
                  {item.hint && (
                    <span className="text-text-tertiary ml-2 text-xs">
                      {item.hint}
                    </span>
                  )}
                </span>
                {item.shortcut && (
                  <CommandShortcut>
                    <Kbd>{item.shortcut}</Kbd>
                  </CommandShortcut>
                )}
              </CommandItem>
            ))}
          </CommandGroup>
        ))}
      </CommandList>
      <div className="flex items-center justify-between gap-4 px-4 py-2 border-t border-border-subtle bg-surface-sunken text-text-tertiary text-2xs">
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1">
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd>
            navigate
          </span>
          <span className="flex items-center gap-1">
            <Kbd>↵</Kbd>
            select
          </span>
        </div>
        <span className="flex items-center gap-1">
          <Kbd>ESC</Kbd>
          close
        </span>
      </div>
    </CommandDialog>
  );
}
