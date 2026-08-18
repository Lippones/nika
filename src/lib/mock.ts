/**
 * Núcleo falso, só para `npm run dev` no navegador — dentro do Tauri este
 * arquivo nunca é usado. Serve para olhar o design sem compilar o app: simula um
 * bootstrap, entrega um circuito e um log plausíveis e faz o tráfego subir.
 */
import type {
  Circuit,
  Config,
  DiscordMode,
  DiscordStatus,
  ExitIp,
  TorStatus,
  Traffic,
} from "./types";

const BOOT: Array<[number, string]> = [
  [5, "Connecting to directory server"],
  [14, "Handshaking with directory server"],
  [25, "Asking for networkstatus consensus"],
  [45, "Loading relay descriptors"],
  [68, "Loading relay descriptors"],
  [80, "Connecting to the Tor network"],
  [95, "Establishing a Tor circuit"],
  [100, "Done"],
];

const CIRCUIT: Circuit = {
  id: "7",
  path: [
    { nickname: "Unnamed0Relay42", fingerprint: "A9C039A5FD1C7A62B25AC0F02F3A4C5B7D91E3F8", country: "DE" },
    { nickname: "quetzalcoatlus", fingerprint: "3B7E1D06C4488F2A9D5E70B1C82F4A6D0E93B517", country: "NL" },
    { nickname: "ForPrivacyNET", fingerprint: "F04B2C9A17D6E385B0C1A72E4D98F6053BA2C1D7", country: "FI" },
  ],
};

// No preview web a janela principal é o padrão; `?onboarding` na URL força a
// primeira execução para revisar as telas de boas-vindas.
const PREVIEW_ONBOARDING =
  typeof location !== "undefined" &&
  new URLSearchParams(location.search).has("onboarding");

let config: Config = {
  socksPort: 9050,
  httpPort: 9080,
  controlPort: 9051,
  autostart: true,
  autoConnect: false,
  discord: { mode: "off", reapplyOnStart: true, allowClose: true },
  onboarded: !PREVIEW_ONBOARDING,
};

/** Um Discord estável instalado, componente já baixado, proxy desligado. */
let discord: DiscordStatus = {
  component: "ready",
  componentVersion: "0.1.0",
  installs: [
    {
      flavor: "stable",
      label: "Discord",
      baseDir: "C:\\Users\\f\\AppData\\Local\\Discord\\",
      exeName: "Discord.exe",
      updateExe: "C:\\Users\\f\\AppData\\Local\\Discord\\Update.exe",
      appDirs: [
        {
          path: "C:\\Users\\f\\AppData\\Local\\Discord\\app-1.0.9186",
          version: "1.0.9186",
          installed: false,
          proxy: null,
        },
      ],
    },
  ],
  running: false,
  effective: "off",
  stale: false,
};

const discordHandlers = new Set<(next: DiscordStatus) => void>();

function pushDiscord(mode: DiscordMode) {
  const proxy =
    mode === "torSocks"
      ? `socks5://127.0.0.1:${config.socksPort}`
      : mode === "torHttp"
        ? `http://127.0.0.1:${config.httpPort}`
        : null;

  discord = {
    ...discord,
    effective: mode,
    stale: false,
    installs: discord.installs.map((install) => ({
      ...install,
      appDirs: install.appDirs.map((dir) => ({
        ...dir,
        installed: mode !== "off",
        proxy,
      })),
    })),
  };

  discordHandlers.forEach((handler) => handler(discord));
  return discord;
}

let status: TorStatus = {
  phase: "stopped",
  bootstrap: 0,
  summary: "Tor parado",
  error: null,
  attempt: 0,
};

let traffic: Traffic = { read: 4_410_000, written: 325_000 };

const statusHandlers = new Set<(next: TorStatus) => void>();
const logHandlers = new Set<(line: string) => void>();
const log: string[] = ["[notice] Tor 0.4.8.12 running on Windows"];

function push(next: Partial<TorStatus>) {
  status = { ...status, ...next };
  statusHandlers.forEach((handler) => handler(status));
}

function emit(line: string) {
  log.push(line);
  logHandlers.forEach((handler) => handler(line));
}

let timer = 0;

function bootstrap() {
  window.clearInterval(timer);
  push({ phase: "starting", bootstrap: 0, summary: "Subindo o tor.exe", error: null });

  let step = 0;
  timer = window.setInterval(() => {
    const entry = BOOT[step];
    if (!entry) {
      window.clearInterval(timer);
      return;
    }

    const [bootstrapPct, summary] = entry;
    emit(`[notice] Bootstrapped ${bootstrapPct}%: ${summary}`);
    push({
      phase: bootstrapPct === 100 ? "connected" : "bootstrapping",
      bootstrap: bootstrapPct,
      summary: `Bootstrapped ${bootstrapPct}% — ${summary}`,
    });
    step += 1;
  }, 700);
}

export const mockApi = {
  getStatus: async () => status,
  getConfig: async () => config,
  setConfig: async (next: Config) => {
    config = next;
    return config;
  },
  connect: async () => bootstrap(),
  disconnect: async () => {
    window.clearInterval(timer);
    emit("[notice] Catching signal TERM, exiting cleanly");
    push({ phase: "stopped", bootstrap: 0, summary: "Tor parado", error: null });
  },
  newIdentity: async () => emit("[notice] Signal NEWNYM received"),
  getLogs: async () => log,
  getCircuit: async () => (status.phase === "connected" ? CIRCUIT : null),
  getTraffic: async () => {
    traffic = { read: traffic.read + 31_000, written: traffic.written + 4_200 };
    return traffic;
  },
  checkExitIp: async (): Promise<ExitIp> => ({ ip: "185.220.101.7", isTor: true }),
  discordStatus: async () => discord,
  discordInstall: async (mode: DiscordMode) => pushDiscord(mode),
  discordUninstall: async () => pushDiscord("off"),
  discordRelaunch: async () => discord,
  copyText: async () => undefined,
  quit: async () => undefined,
};

export function mockOnStatus(handler: (next: TorStatus) => void) {
  statusHandlers.add(handler);
  return () => statusHandlers.delete(handler);
}

export function mockOnLog(handler: (line: string) => void) {
  logHandlers.add(handler);
  return () => logHandlers.delete(handler);
}

export function mockOnDiscord(handler: (next: DiscordStatus) => void) {
  discordHandlers.add(handler);
  return () => discordHandlers.delete(handler);
}

/** No preview web ninguém quer clicar em "Conectar" para ver o estado cheio. */
export function startMockPreview() {
  window.setTimeout(bootstrap, 400);
}
