import { type InputHTMLAttributes } from "react";

export interface TextInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange"> {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  error?: string;
}

export function TextInput({
  value,
  onChange,
  label,
  error,
  className = "",
  ...rest
}: TextInputProps) {
  return (
    <div className={`flex flex-col gap-1.5 ${className}`}>
      {label && (
        <label className="text-xs font-medium text-[var(--color-text-secondary)]">{label}</label>
      )}
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`bg-[var(--color-surface-raised)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-tertiary)] border rounded-[var(--radius-sm)] py-2 px-3 text-sm font-sans outline-none transition-all duration-150 focus:shadow-[0_0_0_2px_var(--color-ember-glow)] ${
          error
            ? "border-[var(--color-error)] focus:border-[var(--color-error)]"
            : "border-[var(--color-border-subtle)] focus:border-[var(--color-ember)]"
        }`}
        {...rest}
      />
      {error && <span className="text-xs text-[var(--color-error)]">{error}</span>}
    </div>
  );
}
