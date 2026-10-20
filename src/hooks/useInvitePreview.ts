import { useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { TunnelProfile } from "../lib/types";

interface InvitePreview {
  /** Convite lido com sucesso. */
  profile: TunnelProfile | null;
  /** O que impede o uso, nas palavras do core. */
  error: string | null;
  /** Há texto, mas a leitura ainda não voltou. */
  checking: boolean;
}

/**
 * Lê o convite enquanto a pessoa cola, sem gravar nada: é o core quem valida,
 * para a tela nunca dizer "ok" para algo que o importar vai recusar.
 */
export function useInvitePreview(text: string): InvitePreview {
  const [state, setState] = useState<InvitePreview>({
    profile: null,
    error: null,
    checking: false,
  });

  useEffect(() => {
    if (text.trim().length === 0) {
      setState({ profile: null, error: null, checking: false });
      return;
    }
    let cancelled = false;
    setState((prev) => ({ ...prev, checking: true }));
    const timer = window.setTimeout(() => {
      api
        .tunnelPreview(text)
        .then((profile) => !cancelled && setState({ profile, error: null, checking: false }))
        .catch(
          (failure) =>
            !cancelled &&
            setState({ profile: null, error: errorMessage(failure), checking: false }),
        );
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [text]);

  return state;
}
