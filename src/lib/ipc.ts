/**
 * Única porta de entrada para o core. Nenhum componente chama `invoke` direto:
 * assim os nomes dos comandos ficam em um lugar só e tipados.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { mockApi, mockOnLog, mockOnTunnel, startMockPreview } from "./mock";
import type { Config, TunnelProfile, TunnelStatus } from "./types";

const EVENT_LOG = "nika://log";
const EVENT_TUNNEL = "nika://tunnel";

/** Fora do Tauri (ou seja, `npm run dev` no navegador) não existe core. */
const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const core = {
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config: Config) => invoke<Config>("set_config", { config }),
  getLogs: () => invoke<string[]>("get_logs"),
  tunnelStatus: () => invoke<TunnelStatus>("tunnel_status"),
  tunnelPreview: (text: string) => invoke<TunnelProfile>("tunnel_preview", { text }),
  tunnelImport: (text: string) => invoke<TunnelStatus>("tunnel_import", { text }),
  tunnelForget: () => invoke<TunnelStatus>("tunnel_forget"),
  tunnelStart: () => invoke<TunnelStatus>("tunnel_start"),
  tunnelStop: () => invoke<TunnelStatus>("tunnel_stop"),
  relaunchElevated: () => invoke<void>("relaunch_elevated"),
  hideWindow: () => invoke<void>("hide_window"),
  minimizeWindow: () => invoke<void>("minimize_window"),
  quit: () => invoke<void>("quit"),
};

export const api = IN_TAURI ? core : mockApi;

if (!IN_TAURI) startMockPreview();

export function onLog(handler: (line: string) => void): Promise<UnlistenFn> {
  if (!IN_TAURI) return Promise.resolve(mockOnLog(handler) as UnlistenFn);
  return listen<string>(EVENT_LOG, (event) => handler(event.payload));
}

export function onTunnel(handler: (status: TunnelStatus) => void): Promise<UnlistenFn> {
  if (!IN_TAURI) return Promise.resolve(mockOnTunnel(handler) as UnlistenFn);
  return listen<TunnelStatus>(EVENT_TUNNEL, (event) => handler(event.payload));
}

/** O core serializa todo erro como string; qualquer outra coisa é bug nosso. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "erro inesperado";
}
