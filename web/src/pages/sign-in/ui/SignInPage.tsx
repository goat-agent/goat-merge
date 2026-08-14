import type { Trouble } from "@/shared/api";
import { Button } from "@/shared/ui";

export function SignInPage({ setUp, trouble }: { setUp: boolean; trouble: Trouble | null }) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-80 space-y-4 rounded-xl bg-sunken p-6 ring-1 ring-hairline-strong">
        <h1 className="text-title text-ink">
          <span className="font-mono">goat</span>
          <span className="mx-1 text-ink-faint/50">/</span>
          <span className="font-mono text-ink-soft">merge</span>
        </h1>
        {trouble ? (
          <p className="text-ui text-warning">{trouble.message}</p>
        ) : (
          <p className="text-ui text-ink-faint">
            {setUp
              ? "Sign in with the GitHub account you use for these repositories."
              : "There is no GitHub App yet, so there is nothing to sign in to."}
          </p>
        )}
        <Button
          tone="primary"
          className="w-full justify-center"
          href={setUp ? "/auth/github" : "/setup"}
        >
          {setUp ? "Sign in with GitHub" : "Set goat-merge up"}
        </Button>
      </div>
    </div>
  );
}
