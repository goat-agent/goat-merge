import { useEffect, useState } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();
let source: EventSource | null = null;

function open() {
  if (source) return;
  const opened = new EventSource("/api/events");
  opened.addEventListener("changed", () => {
    for (const listener of listeners) listener();
  });
  opened.onerror = () => {
    opened.close();
    if (source === opened) source = null;
  };
  source = opened;
}

function closeIfNobodyIsListening() {
  if (listeners.size > 0) return;
  source?.close();
  source = null;
}

export function useLive(): number {
  const [beat, setBeat] = useState(0);

  useEffect(() => {
    const listener = () => setBeat((was) => was + 1);
    listeners.add(listener);
    open();
    return () => {
      listeners.delete(listener);
      closeIfNobodyIsListening();
    };
  }, []);

  return beat;
}

export function useEvery(seconds: number): number {
  const [beat, setBeat] = useState(0);
  useEffect(() => {
    const timer = window.setInterval(() => setBeat((was) => was + 1), seconds * 1000);
    return () => window.clearInterval(timer);
  }, [seconds]);
  return beat;
}
