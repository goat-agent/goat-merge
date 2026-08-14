import {
  Ban,
  CheckCircle2,
  CircleDashed,
  Clock,
  GitMerge,
  Loader2,
  ShieldAlert,
  XCircle,
} from "lucide-react";
import type { ComponentType } from "react";

import type { Status } from "@/shared/api";
import { cn } from "@/shared/lib";

type Look = { icon: ComponentType<{ className?: string }>; tint: string; turning: boolean };

const looks: Record<Status, Look> = {
  "Not queued": { icon: CircleDashed, tint: "text-ink-faint", turning: false },
  Waiting: { icon: Clock, tint: "text-ink-faint", turning: false },
  Blocked: { icon: ShieldAlert, tint: "text-warning", turning: false },
  Queued: { icon: Clock, tint: "text-ink-soft", turning: false },
  Preparing: { icon: Loader2, tint: "text-ink-soft", turning: true },
  Checking: { icon: Loader2, tint: "text-ink-soft", turning: true },
  Merging: { icon: GitMerge, tint: "text-ink-soft", turning: false },
  Merged: { icon: CheckCircle2, tint: "text-success", turning: false },
  Failed: { icon: XCircle, tint: "text-danger", turning: false },
  Cancelled: { icon: Ban, tint: "text-ink-faint", turning: false },
};

export function Standing({ status }: { status: Status }) {
  const look = looks[status];
  const Icon = look.icon;
  return (
    <span className={cn("inline-flex items-center gap-1.5 text-ui", look.tint)}>
      <Icon
        className={cn(
          "size-3.5 shrink-0",
          look.turning && "animate-spin motion-reduce:animate-none",
        )}
      />
      {status}
    </span>
  );
}
