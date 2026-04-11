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
        <label className="text-xs font-medium text-text-secondary">{label}</label>
      )}
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`bg-surface-raised text-text-primary placeholder:text-text-tertiary border rounded-sm py-2 px-3 text-sm font-sans outline-none transition-all duration-150 focus:shadow-[0_0_0_2px_var(--color-ember-glow)] ${
          error
            ? "border-error focus:border-error"
            : "border-border-subtle focus:border-ember"
        }`}
        {...rest}
      />
      {error && <span className="text-xs text-error">{error}</span>}
    </div>
  );
}
