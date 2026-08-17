import { useEffect, useState } from "react";

import { api, onStatus } from "../lib/ipc";
import { INITIAL_STATUS, type TorStatus } from "../lib/types";

/**
 * Estado do Tor: uma leitura inicial e, daí em diante, só o que o core empurra.
 * Nada de polling.
 */
export function useTorStatus(): TorStatus {
  const [status, setStatus] = useState<TorStatus>(INITIAL_STATUS);

  useEffect(() => {
    let mounted = true;
    const unlisten = onStatus((next) => mounted && setStatus(next));

    api.getStatus().then((initial) => {
      if (mounted) setStatus(initial);
    });

    return () => {
      mounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return status;
}
