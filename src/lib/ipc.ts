/**
 * Única porta de entrada para o core. Nenhum componente chama `invoke` direto:
 * assim os nomes dos comandos ficam em um lugar só e tipados.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Circuit, Config, ExitIp, TorStatus, Traffic } from "./types";

const EVENT_STATUS = "nika://status";
const EVENT_LOG = "nika://log";

export const api = {
  getStatus: () => invoke<TorStatus>("get_status"),
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config: Config) => invoke<Config>("set_config", { config }),
  connect: () => invoke<void>("connect"),
  disconnect: () => invoke<void>("disconnect"),
  newIdentity: () => invoke<void>("new_identity"),
  getLogs: () => invoke<string[]>("get_logs"),
  getCircuit: () => invoke<Circuit | null>("get_circuit"),
  getTraffic: () => invoke<Traffic>("get_traffic"),
  checkExitIp: () => invoke<ExitIp>("check_exit_ip"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  quit: () => invoke<void>("quit"),
};

export function onStatus(handler: (status: TorStatus) => void): Promise<UnlistenFn> {
  return listen<TorStatus>(EVENT_STATUS, (event) => handler(event.payload));
}

export function onLog(handler: (line: string) => void): Promise<UnlistenFn> {
  return listen<string>(EVENT_LOG, (event) => handler(event.payload));
}

/** O core serializa todo erro como string; qualquer outra coisa é bug nosso. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "erro inesperado";
}
