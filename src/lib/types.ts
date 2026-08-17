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

/** Espelha `Phase::is_active` no core. */
export function isActive(phase: Phase): boolean {
  return (
    phase === "starting" ||
    phase === "bootstrapping" ||
    phase === "connected" ||
    phase === "retrying"
  );
}
