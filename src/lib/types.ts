/**
 * Espelho dos tipos do core em Rust (`src-tauri/src/state.rs`,
 * `config.rs`, `control/info.rs`). Mudou lá, muda aqui.
 */

export type Phase =
  | "stopped"
  | "starting"
  | "bootstrapping"
  | "connected"
  | "retrying"
  | "failed";

export interface TorStatus {
  phase: Phase;
  /** 0–100 */
  bootstrap: number;
  summary: string;
  error: string | null;
  attempt: number;
}

export interface Config {
  socksPort: number;
  httpPort: number;
  controlPort: number;
  autostart: boolean;
  autoConnect: boolean;
  discord: DiscordConfig;
  /** A janela de boas-vindas já foi concluída. `false` → a UI abre no onboarding. */
  onboarded: boolean;
}

export interface DiscordConfig {
  mode: DiscordMode;
  reapplyOnStart: boolean;
  allowClose: boolean;
}

/** Espelha `discord::Mode`. `torHttp` é o padrão — ver docs/discord-proxy.md D-03. */
export type DiscordMode = "off" | "torHttp" | "torSocks";

/** Espelha `discord::Component`. */
export type DiscordComponent = "missing" | "ready";

export interface DiscordAppDir {
  path: string;
  version: string;
  installed: boolean;
  proxy: string | null;
}

export interface DiscordInstall {
  flavor: "stable" | "canary" | "ptb";
  label: string;
  baseDir: string;
  exeName: string;
  updateExe: string | null;
  appDirs: DiscordAppDir[];
}

export interface DiscordStatus {
  component: DiscordComponent;
  componentVersion: string | null;
  installs: DiscordInstall[];
  running: boolean;
  /** Modo lido do disco, não o da config. */
  effective: DiscordMode;
  /** Instalado, mas apontando para porta diferente da atual. */
  stale: boolean;
}

export interface Relay {
  nickname: string;
  fingerprint: string;
  country: string | null;
}

export interface Circuit {
  id: string;
  path: Relay[];
}

export interface Traffic {
  read: number;
  written: number;
}

export interface ExitIp {
  ip: string;
  isTor: boolean;
}

export const INITIAL_STATUS: TorStatus = {
  phase: "stopped",
  bootstrap: 0,
  summary: "carregando…",
  error: null,
  attempt: 0,
};

export const INITIAL_DISCORD: DiscordStatus = {
  component: "missing",
  componentVersion: null,
  installs: [],
  running: false,
  effective: "off",
  stale: false,
};

/** Espelha `Phase::is_active` no core. */
export function isActive(phase: Phase): boolean {
  return (
    phase === "starting" ||
    phase === "bootstrapping" ||
    phase === "connected" ||
    phase === "retrying"
  );
}
