/**
 * Espelho dos tipos do core em Rust (`src-tauri/src/tunnel/mod.rs`,
 * `config.rs`). Mudou lá, muda aqui.
 */

export interface Config {
  autostart: boolean;
  tunnel: TunnelConfig;
  /** A tela de boas-vindas já foi concluída. `false` → a UI abre no onboarding. */
  onboarded: boolean;
}

/** Espelha `config::TunnelConfig`. */
export interface TunnelConfig {
  autoStart: boolean;
}

/** Espelha `tunnel::Phase`. */
export type TunnelPhase = "off" | "starting" | "up" | "failed";

/** Espelha `tunnel::ProfileSummary` — nunca inclui a chave privada. */
export interface TunnelProfile {
  address: string;
  endpoint: string;
  peerPublicKey: string;
  mtu: number;
  /** `# Nika-Server:` — como o dono chamou a saída ("Filipe · Santiago"). */
  serverLabel: string | null;
  /** `# Nika-Peer:` — para quem o convite foi gerado. */
  peerName: string | null;
}

/** Espelha `tunnel::TunnelStatus` (docs/discord-tunnel.md §7.1). */
export interface TunnelStatus {
  phase: TunnelPhase;
  component: "missing" | "ready";
  elevated: boolean;
  supported: boolean;
  profile: TunnelProfile | null;
  lastHandshakeSecs: number | null;
  txBytes: number;
  rxBytes: number;
  flows: number;
  error: string | null;
}

export const INITIAL_TUNNEL: TunnelStatus = {
  phase: "off",
  component: "missing",
  elevated: false,
  supported: true,
  profile: null,
  lastHandshakeSecs: null,
  txBytes: 0,
  rxBytes: 0,
  flows: 0,
  error: null,
};

/** Sem handshake por tanto tempo, o servidor está fora ou o convite foi revogado. */
export const SILENT_AFTER_SECS = 180;

export function isSilent(status: TunnelStatus): boolean {
  return (
    status.phase === "up" &&
    status.lastHandshakeSecs !== null &&
    status.lastHandshakeSecs > SILENT_AFTER_SECS
  );
}
