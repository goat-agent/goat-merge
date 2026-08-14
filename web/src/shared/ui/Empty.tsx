import type { ReactNode } from "react";

export function Empty({ children }: { children: ReactNode }) {
  return <p className="px-1 py-8 text-center text-ui text-ink-faint">{children}</p>;
}
