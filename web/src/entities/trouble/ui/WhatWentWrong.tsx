import {
  Bug,
  CircleDashed,
  CloudOff,
  FileWarning,
  Hourglass,
  Info,
  ListX,
  Lock,
  LogIn,
  PackageOpen,
  PowerOff,
  WifiOff,
  Wrench,
  XCircle,
} from "lucide-react";
import type { ComponentType } from "react";
import { Link } from "react-router-dom";

import type { Kind, Trouble } from "@/shared/api";
import { cn } from "@/shared/lib";
import { Button } from "@/shared/ui";

type Offer = "nothing" | "again" | "reload" | { follow: string };

type Look = {
  icon: ComponentType<{ className?: string }>;
  tint: string;
  offer: Offer;
};

const looks: Record<Kind, Look> = {
  not_signed_in: {
    icon: LogIn,
    tint: "text-warning",
    offer: { follow: "Sign in with GitHub" },
  },
  not_set_up: {
    icon: Wrench,
    tint: "text-warning",
    offer: { follow: "Set goat-merge up" },
  },
  not_installed_here: {
    icon: PackageOpen,
    tint: "text-warning",
    offer: { follow: "Install it on GitHub" },
  },
  not_allowed: { icon: Lock, tint: "text-warning", offer: "nothing" },
  no_queue_here: {
    icon: ListX,
    tint: "text-ink-faint",
    offer: { follow: "Open settings" },
  },
  never_queued: {
    icon: CircleDashed,
    tint: "text-ink-faint",
    offer: { follow: "Open it on GitHub" },
  },
  queue_is_off: {
    icon: PowerOff,
    tint: "text-warning",
    offer: { follow: "Open settings" },
  },
  github_refused: { icon: XCircle, tint: "text-danger", offer: "again" },
  github_is_rate_limiting: { icon: Hourglass, tint: "text-warning", offer: "again" },
  github_is_unreachable: { icon: CloudOff, tint: "text-warning", offer: "again" },
  configuration_is_invalid: {
    icon: FileWarning,
    tint: "text-danger",
    offer: { follow: "Open the file on GitHub" },
  },
  reason_required: { icon: Info, tint: "text-warning", offer: "nothing" },
  malformed_request: { icon: Bug, tint: "text-danger", offer: "again" },
  no_such_endpoint: { icon: Bug, tint: "text-danger", offer: "reload" },
  broken: { icon: Bug, tint: "text-danger", offer: "again" },
  cannot_reach_the_server: { icon: WifiOff, tint: "text-warning", offer: "again" },
  answer_was_not_json: { icon: Bug, tint: "text-danger", offer: "again" },
};

export function WhatWentWrong({
  trouble,
  place = "page",
  onAgain,
}: {
  trouble: Trouble;
  place?: "page" | "banner";
  onAgain?: (() => void) | undefined;
}) {
  const look = looks[trouble.kind];
  const Icon = look.icon;
  const said = (
    <>
      <Icon className={cn("mt-0.5 size-3.5 shrink-0", look.tint)} />
      <span className="min-w-0">
        {trouble.message}
        {trouble.incident ? (
          <span className="ml-1 font-mono text-mono text-ink-faint">Quote {trouble.incident}</span>
        ) : null}
      </span>
    </>
  );

  if (place === "banner") {
    return (
      <div className="flex shrink-0 items-start gap-2 border-b border-hairline bg-sunken px-4 py-2 text-ui">
        {said}
        <span className="ml-auto shrink-0">
          <Doing look={look} trouble={trouble} onAgain={onAgain} />
        </span>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-1 items-center justify-center p-6">
      <div className="max-w-measure space-y-3">
        <p className="flex items-start gap-2 text-ui">{said}</p>
        <Doing look={look} trouble={trouble} onAgain={onAgain} />
      </div>
    </div>
  );
}

function Doing({
  look,
  trouble,
  onAgain,
}: {
  look: Look;
  trouble: Trouble;
  onAgain: (() => void) | undefined;
}) {
  if (look.offer === "nothing") return null;
  if (look.offer === "reload") {
    return <Button onClick={() => window.location.reload()}>Reload</Button>;
  }
  if (look.offer === "again") {
    return onAgain ? <Button onClick={onAgain}>Try again</Button> : null;
  }

  if (!trouble.goTo) return null;

  const dressed = "text-ui text-primary underline-offset-2 hover:underline";
  if (trouble.goTo.startsWith("/") && trouble.kind !== "not_signed_in") {
    return (
      <Link className={dressed} to={trouble.goTo}>
        {look.offer.follow}
      </Link>
    );
  }
  return (
    <a
      className={dressed}
      href={trouble.goTo}
      target={trouble.goTo.startsWith("/") ? undefined : "_blank"}
      rel="noreferrer"
    >
      {look.offer.follow}
    </a>
  );
}
