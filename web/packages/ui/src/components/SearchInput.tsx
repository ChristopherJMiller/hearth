import { type InputHTMLAttributes } from "react";
import { Search } from "lucide-react";
import { cn } from "../lib/utils";

export interface SearchInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange" | "value" | "type"> {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

export function SearchInput({
  value,
  onChange,
  placeholder = "Search...",
  className,
  ...rest
}: SearchInputProps) {
  return (
    <div className={cn("relative flex items-center", className)}>
      <Search
        className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 size-4 text-text-tertiary"
        aria-hidden="true"
      />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-surface-raised text-text-primary placeholder:text-text-tertiary border border-border-subtle rounded-sm py-2.5 pl-10 pr-3 text-sm font-sans outline-none transition-all duration-150 focus:border-ember focus:shadow-[0_0_0_2px_var(--color-ember-glow)]"
        {...rest}
      />
    </div>
  );
}
