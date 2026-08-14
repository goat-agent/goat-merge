import { useCallback, useEffect, useRef, useState } from "react";

import { Trouble } from "@/shared/api";

export type Asked<T> = {
  answer: T | null;
  trouble: Trouble | null;
  firstLoad: boolean;
  refreshing: boolean;
  again: () => void;
};

type Holding<T> = { about: string; answer: T | null; trouble: Trouble | null };

export function useAsked<T>(
  ask: () => Promise<T>,
  about: (string | number | null)[],
  when = 0,
): Asked<T> {
  const naming = JSON.stringify(about);
  const [holding, setHolding] = useState<Holding<T>>({
    about: naming,
    answer: null,
    trouble: null,
  });
  const [asking, setAsking] = useState(true);
  const [round, setRound] = useState(0);
  const again = useCallback(() => setRound((was) => was + 1), []);

  const latest = useRef(ask);
  latest.current = ask;

  if (holding.about !== naming) {
    setHolding({ about: naming, answer: null, trouble: null });
    setAsking(true);
  }

  useEffect(() => {
    let stale = false;
    setAsking(true);
    latest
      .current()
      .then((given) => {
        if (!stale) setHolding({ about: naming, answer: given, trouble: null });
      })
      .catch((problem: unknown) => {
        if (stale) return;
        const trouble =
          problem instanceof Trouble
            ? problem
            : new Trouble("broken", "The console could not finish that.", null, null);
        setHolding((held) => ({ ...held, trouble }));
      })
      .finally(() => {
        if (!stale) setAsking(false);
      });
    return () => {
      stale = true;
    };
  }, [naming, when, round]);

  const settled = holding.about === naming ? holding : { answer: null, trouble: null };
  return {
    answer: settled.answer,
    trouble: settled.trouble,
    firstLoad: asking && settled.answer === null && settled.trouble === null,
    refreshing: asking && settled.answer !== null,
    again,
  };
}
