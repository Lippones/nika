import { useCallback, useState } from "react";

import { errorMessage } from "../lib/ipc";

interface Action {
  run: (task: () => Promise<unknown>) => Promise<void>;
  pending: boolean;
  error: string | null;
  clear: () => void;
}

/**
 * Envolve uma chamada ao core com estado de "em andamento" e erro, que é o que
 * todo botão desta UI precisa.
 */
export function useAction(): Action {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (task: () => Promise<unknown>) => {
    setPending(true);
    setError(null);
    try {
      await task();
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setPending(false);
    }
  }, []);

  return { run, pending, error, clear: () => setError(null) };
}
