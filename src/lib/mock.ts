/**
 * Núcleo falso, só para `npm run dev` no navegador — dentro do Tauri este
 * arquivo nunca é usado. Serve para olhar o design sem compilar o app.
 *
 * Parâmetros da URL para o preview:
 *   ?onboarding           primeira execução
 *   ?onboarding&step=invite[&invite=<texto>]   passo do convite, opcionalmente preenchido
 *   ?profile              janela principal com convite já guardado
 *   ?profile&up           idem, com a saída no ar
 */
import type { Config, TunnelProfile, TunnelStatus } from "./types";

const params =
  typeof location !== "undefined" ? new URLSearchParams(location.search) : new URLSearchParams();

const PROFILE: TunnelProfile = {
  address: "10.66.0.2",
  endpoint: "203.0.113.9:51820",
  peerPublicKey: "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=",
  mtu: 1380,
  serverLabel: "Filipe · Santiago",
  peerName: "joao",
};

let config: Config = {
  autostart: true,
  tunnel: { autoStart: false },
  onboarded: !params.has("onboarding"),
};

/** Driver presente, processo elevado; convite conforme a URL. */
let tunnel: TunnelStatus = {
  phase: "off",
  component: "ready",
  elevated: true,
  supported: true,
  profile: params.has("profile") ? PROFILE : null,
  lastHandshakeSecs: null,
  txBytes: 0,
  rxBytes: 0,
  flows: 0,
  error: null,
};

/** Aceita o convite de uma linha ou um .conf com `[Peer]`; o resto é erro. */
function mockPreview(text: string): TunnelProfile {
  const trimmed = text.trim();
  if (!trimmed.startsWith("nika://convite/") && !trimmed.includes("[Peer]")) {
    throw "o convite é inválido: falta PublicKey em [Peer]";
  }
  return PROFILE;
}

const tunnelHandlers = new Set<(next: TunnelStatus) => void>();
const logHandlers = new Set<(line: string) => void>();
const log: string[] = ["[nika] preview web — núcleo falso"];
let ticker = 0;

function pushTunnel(next: Partial<TunnelStatus>) {
  tunnel = { ...tunnel, ...next };
  tunnelHandlers.forEach((handler) => handler(tunnel));
  return tunnel;
}

function emit(line: string) {
  log.push(line);
  logHandlers.forEach((handler) => handler(line));
}

function start() {
  if (!tunnel.profile) throw "nenhum convite guardado — cole o convite que você recebeu";
  pushTunnel({ phase: "starting", error: null });
  emit("[nika] saída do Discord ligada");
  window.clearInterval(ticker);
  window.setTimeout(() => pushTunnel({ phase: "up", lastHandshakeSecs: 0 }), 600);
  ticker = window.setInterval(() => {
    if (tunnel.phase !== "up") return;
    pushTunnel({
      lastHandshakeSecs: (tunnel.lastHandshakeSecs ?? 0) + 1,
      txBytes: tunnel.txBytes + 48_000,
      rxBytes: tunnel.rxBytes + 210_000,
      flows: Math.min(tunnel.flows + 1, 6),
    });
  }, 1000);
  return tunnel;
}

export const mockApi = {
  getConfig: async () => config,
  setConfig: async (next: Config) => {
    config = next;
    return config;
  },
  getLogs: async () => log,
  tunnelStatus: async () => tunnel,
  tunnelPreview: async (text: string) => mockPreview(text),
  tunnelImport: async (text: string) => pushTunnel({ error: null, profile: mockPreview(text) }),
  tunnelForget: async () => pushTunnel({ profile: null, phase: "off" }),
  tunnelStart: async () => start(),
  tunnelStop: async () => {
    window.clearInterval(ticker);
    emit("[nika] saída do Discord desligada");
    return pushTunnel({ phase: "off", flows: 0 });
  },
  relaunchElevated: async () => {
    pushTunnel({ elevated: true });
  },
  hideWindow: async () => undefined,
  minimizeWindow: async () => undefined,
  quit: async () => undefined,
};

export function mockOnLog(handler: (line: string) => void) {
  logHandlers.add(handler);
  return () => logHandlers.delete(handler);
}

export function mockOnTunnel(handler: (next: TunnelStatus) => void) {
  tunnelHandlers.add(handler);
  return () => tunnelHandlers.delete(handler);
}

/** `?profile&up` liga sozinho, para ver o estado cheio sem clicar. */
export function startMockPreview() {
  if (params.has("up")) window.setTimeout(() => start(), 400);
}
