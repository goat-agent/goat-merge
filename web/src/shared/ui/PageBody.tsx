import type { ReactNode } from "react";

import { cn } from "@/shared/lib";

export function PageBody({
  children,
  narrow = false,
  padded = true,
}: {
  children: ReactNode;
  narrow?: boolean;
  padded?: boolean;
}) {
  return (
    <div className={cn("flex min-h-0 flex-1 flex-col overflow-y-auto", padded && "p-6")}>
      <div className={cn("flex min-h-0 flex-1 flex-col", narrow && "mx-auto w-full max-w-measure")}>
        {children}
      </div>
    </div>
  );
}
