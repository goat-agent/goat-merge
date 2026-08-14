import type { InputHTMLAttributes } from "react";

import { cn } from "@/shared/lib";

export function Field({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        "h-control rounded-md bg-fill-subtle px-2.5 text-ui text-ink",
        "placeholder:text-ink-faint",
        className,
      )}
      {...rest}
    />
  );
}
