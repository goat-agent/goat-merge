import { ChevronDown } from "lucide-react";

import { cn } from "@/shared/lib";

export type Choice = { value: string; label: string };

export function Select({
  label,
  value,
  options,
  onPick,
  className,
}: {
  label: string;
  value: string;
  options: Choice[];
  onPick: (picked: string) => void;
  className?: string;
}) {
  const known = options.some((option) => option.value === value)
    ? options
    : [{ value, label: value }, ...options];
  return (
    <span className={cn("relative inline-flex items-center", className)}>
      <select
        aria-label={label}
        className={cn(
          "h-control appearance-none rounded-md bg-fill-subtle pr-7 pl-2.5 text-ui text-ink",
          "transition-colors hover:bg-fill-hover",
        )}
        value={value}
        onChange={(event) => onPick(event.target.value)}
      >
        {known.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <ChevronDown className="pointer-events-none absolute right-2 size-3.5 text-ink-faint" />
    </span>
  );
}
