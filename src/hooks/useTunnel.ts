import { useEffect, useState } from "react";

import { api, onTunnel } from "../lib/ipc";
import { INITIAL_TUNNEL, type TunnelStatus } from "../lib/types";

interface TunnelState {
  status: TunnelStatus;
  /** A primeira leitura ainda não voltou. */
  loading: boolean;
}

/**
 * Estado da saída própria do Discord (docs/discord-tunnel.md). O core emite a
 * cada mudança e, com o túnel no ar, uma vez por segundo com as estatísticas.
 */
export function useTunnel(): TunnelState {
  const [status, setStatus] = useState<TunnelStatus>(INITIAL_TUNNEL);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    const unlisten = onTunnel((next) => {
      if (!mounted) return;
      setStatus(next);
      setLoading(false);
    });

    api
      .tunnelStatus()
      .then((initial) => mounted && setStatus(initial))
      .catch(() => {})
      .finally(() => mounted && setLoading(false));

    return () => {
      mounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return { status, loading };
}
