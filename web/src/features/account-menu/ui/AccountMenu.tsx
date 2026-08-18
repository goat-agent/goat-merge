import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { api } from "@/shared/api";

export function AccountMenu({ login }: { login: string }) {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);

  async function signOut() {
    setOpen(false);
    try {
      await api.logout();
    } finally {
      window.location.assign("/");
    }
  }

  return (
    <div className="relative shrink-0">
      <button
        type="button"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((was) => !was)}
        className="flex h-control items-center gap-1.5 rounded-md px-2 text-ui text-ink-faint transition-colors hover:bg-fill-hover hover:text-ink"
      >
        <span className="max-w-40 truncate">{login}</span>
        <ChevronDown className="size-3.5 shrink-0" />
      </button>
      {open ? (
        <>
          <button
            type="button"
            aria-label="Close this menu"
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div
            role="menu"
            className="absolute right-0 z-20 mt-1 w-56 rounded-lg bg-raised p-1 shadow-lg ring-1 ring-hairline-strong"
          >
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                navigate("/token");
              }}
              className="flex w-full items-center rounded-md px-2 py-1.5 text-left text-ui text-ink-soft transition-colors hover:bg-fill-hover hover:text-ink"
            >
              Show my CLI token
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={signOut}
              className="flex w-full items-center rounded-md px-2 py-1.5 text-left text-ui text-ink-soft transition-colors hover:bg-fill-hover hover:text-ink"
            >
              Sign out
            </button>
          </div>
        </>
      ) : null}
    </div>
  );
}
