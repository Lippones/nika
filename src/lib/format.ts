import type { Config, Phase } from "./types";

export const PHASE_LABEL: Record<Phase, string> = {
  stopped: "Desconectado",
  starting: "Iniciando",
  bootstrapping: "Conectando",
  connected: "Conectado",
  retrying: "Reconectando",
  failed: "Erro",
};

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
