/**
 * Única porta de entrada para o core. Nenhum componente chama `invoke` direto:
 * assim os nomes dos comandos ficam em um lugar só e tipados.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  mockApi,
  mockOnDiscord,
  mockOnLog,
  mockOnStatus,
  startMockPreview,
} from "./mock";
import type {
  Circuit,
  Config,
  DiscordMode,
  DiscordStatus,
  ExitIp,
  TorStatus,
  Traffic,
} from "./types";

const EVENT_STATUS = "nika://status";
const EVENT_LOG = "nika://log";
const EVENT_DISCORD = "nika://discord";

/** Fora do Tauri (ou seja, `npm run dev` no navegador) não existe core. */
const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const core = {
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
  discordStatus: () => invoke<DiscordStatus>("discord_status"),
  discordInstall: (mode: DiscordMode, closeDiscord: boolean, relaunch: boolean) =>
    invoke<DiscordStatus>("discord_install", { mode, closeDiscord, relaunch }),
  discordUninstall: (closeDiscord: boolean, relaunch: boolean) =>
    invoke<DiscordStatus>("discord_uninstall", { closeDiscord, relaunch }),
  discordRelaunch: () => invoke<DiscordStatus>("discord_relaunch"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  quit: () => invoke<void>("quit"),
};

export const api = IN_TAURI ? core : mockApi;

if (!IN_TAURI) startMockPreview();

export function onStatus(handler: (status: TorStatus) => void): Promise<UnlistenFn> {
  if (!IN_TAURI) return Promise.resolve(mockOnStatus(handler) as UnlistenFn);
  return listen<TorStatus>(EVENT_STATUS, (event) => handler(event.payload));
}

export function onLog(handler: (line: string) => void): Promise<UnlistenFn> {
  if (!IN_TAURI) return Promise.resolve(mockOnLog(handler) as UnlistenFn);
  return listen<string>(EVENT_LOG, (event) => handler(event.payload));
}

export function onDiscord(
  handler: (status: DiscordStatus) => void,
): Promise<UnlistenFn> {
  if (!IN_TAURI) return Promise.resolve(mockOnDiscord(handler) as UnlistenFn);
  return listen<DiscordStatus>(EVENT_DISCORD, (event) => handler(event.payload));
}

/** O core serializa todo erro como string; qualquer outra coisa é bug nosso. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "erro inesperado";
}
