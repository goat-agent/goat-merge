import { Pause, Play } from "lucide-react";
import { useState } from "react";

import { api, Trouble } from "@/shared/api";
import { Button } from "@/shared/ui";

export function QueueControls({
  owner,
  name,
  branch,
  paused,
  onChanged,
  onTrouble,
}: {
  owner: string;
  name: string;
  branch: string;
  paused: boolean;
  onChanged: () => void;
  onTrouble: (wrong: Trouble | null) => void;
}) {
  const [busy, setBusy] = useState(false);

  async function flip() {
    setBusy(true);
    onTrouble(null);
    try {
      await (paused ? api.resume(owner, name, branch) : api.pause(owner, name, branch));
      onChanged();
    } catch (problem: unknown) {
      onTrouble(
        problem instanceof Trouble
          ? problem
          : new Trouble("broken", "The console could not finish that.", null, null),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <Button onClick={flip} disabled={busy}>
      {paused ? <Play className="size-3.5" /> : <Pause className="size-3.5" />}
      {paused ? "Resume" : "Pause"}
    </Button>
  );
}
