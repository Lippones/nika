import type { Config, Phase } from "./types";

/**
 * O título do talão vem em duas partes: a palavra que diz o estado e o resto da
 * frase, que a UI esmaece. Caixa alta é aplicada no CSS.
 */
export const PHASE_TITLE: Record<Phase, [string, string]> = {
  stopped: ["Proxy", "parado"],
  starting: ["Subindo", "o Tor"],
  bootstrapping: ["Conectando", "à rede"],
  connected: ["Proxy", "no ar"],
  retrying: ["Reconectando", "à rede"],
  failed: ["Falha", "na conexão"],
};

/** Versão curta, para a barra de cabeçalho do bilhete. */
export const PHASE_TAG: Record<Phase, string> = {
  stopped: "parado",
  starting: "subindo",
  bootstrapping: "conectando",
  connected: "no ar",
  retrying: "reconectando",
  failed: "falha",
};

/** Papel de cada salto do circuito — a ordem é a informação. */
export function hopRole(index: number, total: number): string {
  if (index === total - 1) return "saída";
  return index === 0 ? "guarda" : "meio";
}

export function socksUrl(config: Config): string {
  return `socks5://127.0.0.1:${config.socksPort}`;
}

export function httpUrl(config: Config): string {
  return `http://127.0.0.1:${config.httpPort}`;
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
