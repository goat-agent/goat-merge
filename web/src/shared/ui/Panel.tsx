import type { ReactNode } from "react";

import { cn } from "@/shared/lib";

export function Panel({
  label,
  flush = false,
  children,
}: {
  label?: ReactNode;
  flush?: boolean;
  children: ReactNode;
}) {
  return (
    <section className="space-y-2">
      {label ? (
        <header className="flex h-6 items-center">
          <h2 className="text-caption uppercase text-ink-faint">{label}</h2>
        </header>
      ) : null}
      <div className={cn("overflow-hidden rounded-lg bg-sunken", flush ? null : "p-4")}>
        {children}
      </div>
    </section>
  );
}
