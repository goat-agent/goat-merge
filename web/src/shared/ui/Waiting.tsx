import { Loader2 } from "lucide-react";

export function Waiting({ of, place = "page" }: { of: string; place?: "page" | "banner" }) {
  if (place === "banner") {
    return (
      <div className="h-px shrink-0 animate-pulse bg-hairline-strong motion-reduce:animate-none" />
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-1 items-center justify-center p-6">
      <p className="flex items-center gap-2 text-ui text-ink-faint">
        <Loader2 className="size-3.5 shrink-0 animate-spin motion-reduce:animate-none" />
        Reading {of}…
      </p>
    </div>
  );
}
