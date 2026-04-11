import {
  Select as ShadcnSelect,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { cn } from "../lib/utils";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  label?: string;
  className?: string;
  disabled?: boolean;
  id?: string;
  name?: string;
}

export function Select({
  options,
  value,
  onChange,
  placeholder,
  label,
  className,
  disabled,
  id,
  name,
}: SelectProps) {
  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      {label && (
        <label
          htmlFor={id}
          className="text-xs font-medium text-text-secondary"
        >
          {label}
        </label>
      )}
      <ShadcnSelect
        value={value}
        onValueChange={onChange}
        disabled={disabled}
        name={name}
      >
        <SelectTrigger
          id={id}
          className="w-full bg-surface-raised border-border-subtle text-text-primary"
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent className="bg-surface-popover border-border text-text-primary">
          {options.map((opt) => (
            <SelectItem key={opt.value} value={opt.value}>
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </ShadcnSelect>
    </div>
  );
}
