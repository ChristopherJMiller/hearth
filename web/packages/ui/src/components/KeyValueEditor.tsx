import { useState } from "react";

export interface KeyValueEditorProps {
  value: Record<string, string>;
  onChange: (next: Record<string, string>) => void;
  keyLabel?: string;
  valueLabel?: string;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  monoValues?: boolean;
  addLabel?: string;
  emptyLabel?: string;
}

interface Row {
  id: number;
  key: string;
  value: string;
}

let nextId = 1;

function toRows(map: Record<string, string>): Row[] {
  return Object.entries(map).map(([key, value]) => ({ id: nextId++, key, value }));
}

function toMap(rows: Row[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of rows) {
    if (r.key.trim()) out[r.key.trim()] = r.value;
  }
  return out;
}

export function KeyValueEditor({
  value,
  onChange,
  keyLabel = "Key",
  valueLabel = "Value",
  keyPlaceholder = "name",
  valuePlaceholder = "value",
  monoValues = false,
  addLabel = "Add entry",
  emptyLabel = "No entries. Click \"Add entry\" to create one.",
}: KeyValueEditorProps) {
  // Controlled pattern: rebuild rows from the prop on first render only; edits
  // flow back via onChange with the full map.
  const [rows, setRows] = useState<Row[]>(() => toRows(value));

  const push = (next: Row[]) => {
    setRows(next);
    onChange(toMap(next));
  };

  const update = (id: number, patch: Partial<Row>) => {
    push(rows.map((r) => (r.id === id ? { ...r, ...patch } : r)));
  };

  const remove = (id: number) => push(rows.filter((r) => r.id !== id));

  const add = () => setRows([...rows, { id: nextId++, key: "", value: "" }]);

  return (
    <div className="flex flex-col gap-2.5">
      {rows.length === 0 ? (
        <div
          className="py-4 px-3 rounded-sm bg-surface-sunken text-text-tertiary text-center italic text-xs"
         
        >
          {emptyLabel}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,2fr)_auto] gap-2 uppercase font-semibold text-text-tertiary text-2xs tracking-wide"
              >
            <span>{keyLabel}</span>
            <span>{valueLabel}</span>
            <span aria-hidden="true" className="w-8" />
          </div>
          {rows.map((row) => (
            <div
              key={row.id}
              className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,2fr)_auto] gap-2"
            >
              <input
                type="text"
                value={row.key}
                onChange={(e) => update(row.id, { key: e.target.value })}
                placeholder={keyPlaceholder}
                className="px-3 py-2 rounded-sm bg-surface-sunken border border-border-subtle text-text-primary placeholder:text-text-tertiary focus:outline-none focus:border-ember text-sm"
               
              />
              <input
                type="text"
                value={row.value}
                onChange={(e) => update(row.id, { value: e.target.value })}
                placeholder={valuePlaceholder}
                className={`px-3 py-2 rounded-sm bg-surface-sunken border border-border-subtle text-text-primary placeholder:text-text-tertiary focus:outline-none focus:border-ember ${monoValues ? "font-mono text-xs" : "text-sm"}`}
              />
              <button
                type="button"
                onClick={() => remove(row.id)}
                className="w-9 h-9 flex items-center justify-center rounded-sm text-text-tertiary hover:text-error hover:bg-error-faint transition-colors cursor-pointer"
                aria-label="Remove entry"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
          ))}
        </>
      )}
      <button
        type="button"
        onClick={add}
        className="self-start inline-flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-text-secondary hover:text-ember hover:bg-ember-faint border border-dashed border-border hover:border-border-accent transition-colors cursor-pointer text-xs"
       
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M12 5v14M5 12h14" />
        </svg>
        {addLabel}
      </button>
    </div>
  );
}
