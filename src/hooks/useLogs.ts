import { useEffect, useState } from "react";

import { api, onLog } from "../lib/ipc";

/** Mesmo teto do buffer no core, para a lista não crescer sem fim. */
const MAX_LINES = 500;

export function useLogs(): string[] {
  const [lines, setLines] = useState<string[]>([]);

  useEffect(() => {
    let mounted = true;
    const unlisten = onLog((line) => {
      if (mounted) setLines((current) => [...current, line].slice(-MAX_LINES));
    });

    api.getLogs().then((initial) => {
      if (mounted) setLines(initial.slice(-MAX_LINES));
    });

    return () => {
      mounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return lines;
}
