import type { AnchorHTMLAttributes, ButtonHTMLAttributes } from "react";

import { cn } from "@/shared/lib";

type Tone = "ghost" | "primary" | "danger";

type Props =
  | ({ tone?: Tone; href?: undefined } & ButtonHTMLAttributes<HTMLButtonElement>)
  | ({ tone?: Tone; href: string } & AnchorHTMLAttributes<HTMLAnchorElement>);

const tones: Record<Tone, string> = {
  ghost: "text-ink-faint hover:bg-fill-hover hover:text-ink active:bg-fill-active",
  primary: "bg-primary text-primary-contrast hover:bg-primary-hover",
  danger: "text-danger hover:bg-fill-hover active:bg-fill-active",
};

const control = cn(
  "inline-flex h-control items-center gap-1.5 rounded-md px-3 text-ui transition-colors",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

export function Button({ tone = "ghost", className, ...rest }: Props) {
  if (rest.href === undefined) {
    return <button type="button" className={cn(control, tones[tone], className)} {...rest} />;
  }
  return <a className={cn(control, tones[tone], className)} {...rest} />;
}
