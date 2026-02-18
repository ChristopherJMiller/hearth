import { type InputHTMLAttributes } from "react";

export interface SearchInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange" | "value" | "type"> {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

function SearchIcon() {
  return (
    <svg
      className="absolute left-3 pointer-events-none text-[var(--color-text-tertiary)]"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  );
}

export function SearchInput({
  value,
  onChange,
  placeholder = "Search...",
  className = "",
  ...rest
}: SearchInputProps) {
  return (
    <div className={`relative flex items-center ${className}`}>
      <SearchIcon />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-[var(--color-surface-raised)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-tertiary)] border border-[var(--color-border-subtle)] rounded-[var(--radius-sm)] py-2.5 pl-10 pr-3 text-sm font-sans outline-none transition-all duration-150 focus:border-[var(--color-ember)] focus:shadow-[0_0_0_2px_var(--color-ember-glow)]"
        {...rest}
      />
    </div>
  );
}
