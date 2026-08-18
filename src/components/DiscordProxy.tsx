import { useState } from "react";

import { Band } from "./Band";
import { useAction } from "../hooks/useAction";
import { api } from "../lib/ipc";
import type { Config, DiscordMode, DiscordStatus } from "../lib/types";

interface DiscordProxyProps {
  status: DiscordStatus;
  loading: boolean;
  /** O download e a instalação só fazem sentido com o Tor no ar (RF-33/36). */
  connected: boolean;
  config: Config;
  onSave: (next: Config) => void;
}

const MODES: Array<{ value: Exclude<DiscordMode, "off">; label: string; hint: string }> = [
  { value: "torHttp", label: "HTTP", hint: "recomendado" },
  { value: "torSocks", label: "SOCKS5", hint: "caminho alternativo" },
];

/**
 * RF-27 a RF-41. O Discord não tem configuração de proxy: quem o faz obedecer é
 * um componente de terceiro instalado ao lado do executável. Esta faixa mostra
 * o que está em disco — não o que o app acha que instalou.
 */
export function DiscordProxy({
  status,
  loading,
  connected,
  config,
  onSave,
}: DiscordProxyProps) {
  const action = useAction();
  const [mode, setMode] = useState<Exclude<DiscordMode, "off">>(
    status.effective === "torSocks" ? "torSocks" : "torHttp",
  );
  const [closeDiscord, setCloseDiscord] = useState(config.discord.allowClose);

  if (loading) return null;

  if (status.installs.length === 0) {
    return (
      <Band label="Proxy no Discord">
        <p className="empty">Discord não encontrado neste usuário</p>
      </Band>
    );
  }

  const installed = status.effective !== "off";
  const folders = status.installs.reduce(
    (total, install) => total + install.appDirs.length,
    0,
  );
  const address = status.installs
    .flatMap((install) => install.appDirs)
    .find((dir) => dir.installed)?.proxy;

  return (
    <Band label="Proxy no Discord">
      <ul className="rows">
        <li className="row">
          <span className="row__key">Estado</span>
          <span className="row__value row__value--strong">
            {installed ? `ativo em ${folders} pasta(s)` : "desligado"}
          </span>
        </li>

        {installed && address && (
          <li className="row">
            <span className="row__key">Endereço</span>
            <span className="row__value">{address}</span>
          </li>
        )}

        <li className="row">
          <span className="row__key">Componente</span>
          <span className="row__value">
            {status.component === "ready"
              ? `version.dll ${status.componentVersion ?? ""}`.trim()
              : "não incluído neste build"}
          </span>
        </li>

        <li className="row">
          <span className="row__key">Discord</span>
          <span className="row__value">{status.running ? "aberto" : "fechado"}</span>
        </li>

        {status.installs.map((install) => (
          <li className="row" key={install.baseDir}>
            <span className="row__key">{install.label}</span>
            <span className="row__value">
              {install.appDirs[0]?.version || install.baseDir}
            </span>
          </li>
        ))}
      </ul>

      {action.error && (
        <p className="notice">
          <strong>A ação não foi concluída.</strong>
          {action.error}
        </p>
      )}

      {status.stale && (
        <p className="notice">
          <strong>Endereço desatualizado.</strong>
          As portas mudaram desde a instalação. O Discord só relê o endereço ao
          abrir — reinicie-o para voltar a passar pelo Tor.
        </p>
      )}

      {status.component !== "ready" ? (
        <p className="notice">
          <strong>Componente ausente.</strong>
          Este build do Nika não inclui o <code>version.dll</code> do proxy do
          Discord. Compile o shim (<code>scripts/build-shim.ps1</code>) e gere o
          instalador de novo.
        </p>
      ) : installed ? (
        <div className="actions">
          <button
            type="button"
            className="solid"
            disabled={action.pending}
            onClick={() =>
              void action.run(() => api.discordUninstall(closeDiscord, closeDiscord))
            }
          >
            {action.pending ? "Removendo…" : "Remover"}
          </button>
          {status.stale && (
            <button
              type="button"
              disabled={action.pending}
              onClick={() => void action.run(api.discordRelaunch)}
            >
              Reiniciar o Discord
            </button>
          )}
        </div>
      ) : (
        <>
          <div className="actions">
            {MODES.map(({ value, label, hint }) => (
              <button
                key={value}
                type="button"
                className={mode === value ? "solid" : ""}
                onClick={() => setMode(value)}
              >
                {label} · {hint}
              </button>
            ))}
          </div>

          <div className="actions">
            <button
              type="button"
              className="solid"
              disabled={action.pending || !connected}
              onClick={() =>
                void action.run(() => api.discordInstall(mode, closeDiscord, closeDiscord))
              }
            >
              {action.pending ? "Instalando…" : "Instalar"}
            </button>
            {!connected && <span className="empty">conecte o Tor primeiro</span>}
          </div>
        </>
      )}

      <div className="switches">
        {status.running && (
          <label className="switch">
            <span>Fechar e reabrir o Discord</span>
            <input
              type="checkbox"
              checked={closeDiscord}
              onChange={(event) => setCloseDiscord(event.target.checked)}
            />
          </label>
        )}

        <label className="switch">
          <span>Reaplicar após atualização do Discord</span>
          <input
            type="checkbox"
            checked={config.discord.reapplyOnStart}
            onChange={(event) =>
              onSave({
                ...config,
                discord: { ...config.discord, reapplyOnStart: event.target.checked },
              })
            }
          />
        </label>
      </div>

      {installed ? (
        <p className="notice">
          <strong>Voz não passa pelo Tor.</strong>
          A voz do Discord é UDP e o Tor só transporta TCP: entrar em um canal de
          voz revela seu IP real. O proxy cobre mensagens, gateway e mídia.
        </p>
      ) : (
        <p className="notice">
          <strong>Antes de instalar.</strong>
          O Discord trata IP de saída Tor com hostilidade: captcha em loop,
          verificação por telefone e bloqueio de conta são resultados comuns. E
          chamadas de voz continuarão saindo pelo seu IP real.
        </p>
      )}
    </Band>
  );
}
