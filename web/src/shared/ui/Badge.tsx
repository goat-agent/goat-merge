import type { ReactNode } from "react";

import { cn } from "@/shared/lib";

type Tone = "quiet" | "success" | "warning" | "danger";

const tones: Record<Tone, string> = {
  quiet: "text-ink-faint",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
};

export function Badge({ tone = "quiet", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-sm bg-fill-hover px-1.5 py-0.5 text-caption uppercase",
        tones[tone],
      )}
    >
      {children}
    </span>
  );
}
