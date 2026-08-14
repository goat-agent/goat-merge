import { Check, Copy } from "lucide-react";
import { useEffect, useState } from "react";

export function Code({ children }: { children: string }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  return (
    <div className="group relative">
      <pre className="overflow-x-auto rounded-lg bg-sunken p-4 pr-12 font-mono text-mono text-ink">
        {children}
      </pre>
      <button
        type="button"
        aria-label={copied ? "Copied" : "Copy"}
        className="absolute top-2 right-2 rounded-md p-1.5 text-ink-faint opacity-0 transition-opacity hover:bg-fill-hover hover:text-ink focus-visible:opacity-100 group-hover:opacity-100 [@media(hover:none)]:opacity-100"
        onClick={() => {
          navigator.clipboard.writeText(children).then(
            () => setCopied(true),
            () => setCopied(false),
          );
        }}
      >
        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      </button>
    </div>
  );
}
