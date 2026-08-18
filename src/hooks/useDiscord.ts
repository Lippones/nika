import { useCallback, useEffect, useState } from "react";

import { api, onDiscord } from "../lib/ipc";
import { INITIAL_DISCORD, type DiscordStatus } from "../lib/types";

interface DiscordState {
  status: DiscordStatus;
  /** Uma varredura ainda não voltou — evita piscar "não encontrei o Discord". */
  loading: boolean;
  refresh: () => void;
}

/**
 * Estado do proxy no Discord. Como o core não faz cache (o disco é a verdade),
 * a varredura é pedida de novo sempre que uma ação pode tê-la invalidado.
 */
export function useDiscord(): DiscordState {
  const [status, setStatus] = useState<DiscordStatus>(INITIAL_DISCORD);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(() => {
    api
      .discordStatus()
      .then(setStatus)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    let mounted = true;
    const unlisten = onDiscord((next) => {
      if (!mounted) return;
      setStatus(next);
      setLoading(false);
    });

    api
      .discordStatus()
      .then((initial) => mounted && setStatus(initial))
      .catch(() => {})
      .finally(() => mounted && setLoading(false));

    return () => {
      mounted = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  return { status, loading, refresh };
}
