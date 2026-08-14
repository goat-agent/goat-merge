import type { ReactNode } from "react";

import { cn } from "@/shared/lib";

export function Well({
  children,
  className,
  id,
}: {
  children: ReactNode;
  className?: string;
  id?: string;
}) {
  return (
    <div id={id} className={cn("space-y-1 rounded-lg bg-sunken p-3", className)}>
      {children}
    </div>
  );
}
