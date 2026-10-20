import type { TunnelPhase, TunnelStatus } from "./types";

/**
 * O título do talão vem em duas partes: a palavra que diz o estado e o resto da
 * frase, que a UI esmaece. Caixa alta é aplicada no CSS.
 */
export const PHASE_TITLE: Record<TunnelPhase, [string, string]> = {
  off: ["Saída", "desligada"],
  starting: ["Ligando", "a saída"],
  up: ["Saída", "no ar"],
  failed: ["Falha", "na saída"],
};

/** Versão curta, para a barra de cabeçalho do bilhete. */
export const PHASE_TAG: Record<TunnelPhase, string> = {
  off: "desligada",
  starting: "ligando",
  up: "no ar",
  failed: "falha",
};

/** "agora", "há 40 s", "há 5 min": quando o servidor respondeu pela última vez. */
export function handshakeLabel(status: TunnelStatus): string {
  if (status.phase !== "up") return "—";
  const secs = status.lastHandshakeSecs;
  if (secs === null) return "aguardando o servidor";
  if (secs < 5) return "agora";
  if (secs < 120) return `há ${secs} s`;
  return `há ${Math.round(secs / 60)} min`;
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}
