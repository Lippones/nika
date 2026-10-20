import { useState } from "react";

import { Band } from "./Band";
import { InviteInput } from "./InviteInput";
import { useAction } from "../hooks/useAction";
import { useInvitePreview } from "../hooks/useInvitePreview";
import { formatBytes } from "../lib/format";
import { api } from "../lib/ipc";
import type { Config, TunnelStatus } from "../lib/types";

interface TunnelProps {
  status: TunnelStatus;
  loading: boolean;
  config: Config;
  onSave: (next: Config) => void;
}

const PHASE_LABEL: Record<TunnelStatus["phase"], string> = {
  off: "desligada",
  starting: "ligando…",
  up: "no ar",
  failed: "falhou",
};

/** Sem handshake por tanto tempo, o servidor está fora ou o convite foi revogado. */
const SILENT_AFTER_SECS = 180;

function handshakeLabel(secs: number | null, phase: TunnelStatus["phase"]): string {
  if (phase !== "up") return "—";
  if (secs === null) return "aguardando o servidor";
  if (secs < 5) return "agora";
  if (secs < 120) return `há ${secs} s`;
  return `há ${Math.round(secs / 60)} min`;
}

/**
 * RF-50 a RF-60 (docs/discord-tunnel.md). Todo o tráfego dos processos do
 * Discord sai pelo servidor de quem te convidou (ou pelo seu); o resto do PC
 * não é tocado. A faixa mostra o que o engine reporta, não o que o app acha
 * que fez.
 */
export function Tunnel({ status, loading, config, onSave }: TunnelProps) {
  const action = useAction();
  const [draft, setDraft] = useState("");
  const preview = useInvitePreview(draft);

  if (loading) return null;

  if (!status.supported) {
    return (
      <Band label="Saída do Discord">
        <p className="empty">disponível só no Windows</p>
      </Band>
    );
  }

  const up = status.phase === "up" || status.phase === "starting";
  const silent =
    status.phase === "up" &&
    status.lastHandshakeSecs !== null &&
    status.lastHandshakeSecs > SILENT_AFTER_SECS;
  const neverAnswered =
    status.phase === "up" && status.lastHandshakeSecs === null && status.flows > 0;

  return (
    <Band label="Saída do Discord">
      {status.profile ? (
        <ul className="rows">
          <li className="row">
            <span className="row__key">Estado</span>
            <span className="row__value row__value--strong">{PHASE_LABEL[status.phase]}</span>
          </li>
          <li className="row">
            <span className="row__key">Servidor</span>
            <span className="row__value">
              {status.profile.serverLabel ?? "sem rótulo"} · {status.profile.endpoint}
            </span>
          </li>
          {status.profile.peerName && (
            <li className="row">
              <span className="row__key">Convite</span>
              <span className="row__value">para {status.profile.peerName}</span>
            </li>
          )}
          <li className="row">
            <span className="row__key">Handshake</span>
            <span className="row__value">
              {handshakeLabel(status.lastHandshakeSecs, status.phase)}
            </span>
          </li>
          {status.phase === "up" && (
            <>
              <li className="row">
                <span className="row__key">Tráfego</span>
                <span className="row__value">
                  ↑ {formatBytes(status.txBytes)} · ↓ {formatBytes(status.rxBytes)}
                </span>
              </li>
              <li className="row">
                <span className="row__key">Fluxos</span>
                <span className="row__value">
                  {status.flows === 0 ? "nenhum ainda · abra o Discord" : `${status.flows} porta(s) do Discord`}
                </span>
              </li>
            </>
          )}
        </ul>
      ) : (
        <InviteInput
          value={draft}
          onChange={setDraft}
          profile={preview.profile}
          error={preview.error}
          checking={preview.checking}
        />
      )}

      {(silent || neverAnswered) && (
        <p className="notice" role="status">
          <strong>Sem resposta do servidor.</strong>
          Ele pode estar desligado, ou o seu convite foi revogado. O Discord não
          conecta enquanto isso. Fale com quem te convidou.
        </p>
      )}

      {(action.error || status.error) && (
        <p className="notice" role="alert">
          <strong>Não foi possível.</strong>
          {action.error ?? status.error}
        </p>
      )}

      {status.component !== "ready" && (
        <p className="notice">
          <strong>Componente ausente.</strong>
          Este build do Nika não inclui o driver WinDivert. Rode{" "}
          <code>scripts/fetch-windivert.ps1</code> e gere o instalador de novo.
        </p>
      )}

      {status.profile && !status.elevated && status.component === "ready" && !up && (
        <p className="notice">
          <strong>Precisa de administrador.</strong>
          Separar o tráfego do Discord do resto do PC exige um driver, e ele só abre
          com permissão de administrador. O Nika reabre pedindo (UAC) e liga em seguida.
        </p>
      )}

      <div className="actions">
        {!status.profile ? (
          <button
            type="button"
            className="solid"
            disabled={action.pending || preview.profile === null}
            onClick={() => void action.run(() => api.tunnelImport(draft).then(() => setDraft("")))}
          >
            {action.pending ? "Guardando…" : "Usar convite"}
          </button>
        ) : up ? (
          <button
            type="button"
            className="solid"
            disabled={action.pending}
            onClick={() => void action.run(api.tunnelStop)}
          >
            {action.pending ? "Desligando…" : "Desligar"}
          </button>
        ) : !status.elevated ? (
          <button
            type="button"
            className="solid"
            disabled={action.pending || status.component !== "ready"}
            onClick={() => void action.run(api.relaunchElevated)}
          >
            Reabrir como administrador
          </button>
        ) : (
          <button
            type="button"
            className="solid"
            disabled={action.pending || status.component !== "ready"}
            onClick={() => void action.run(api.tunnelStart)}
          >
            {action.pending ? "Ligando…" : "Ligar"}
          </button>
        )}

        {status.profile && !up && (
          <button
            type="button"
            className="ghost"
            disabled={action.pending}
            onClick={() => void action.run(api.tunnelForget)}
          >
            Esquecer convite
          </button>
        )}
      </div>

      {status.profile && (
        <div className="switches">
          <label className="switch">
            <span>Ligar ao abrir o Nika (se for admin)</span>
            <input
              type="checkbox"
              checked={config.tunnel.autoStart}
              onChange={(event) =>
                onSave({ ...config, tunnel: { ...config.tunnel, autoStart: event.target.checked } })
              }
            />
          </label>
        </div>
      )}

      <p className="notice">
        <strong>Não é anonimato.</strong>
        O Discord passa a ver o IP do servidor de quem te convidou, inclusive na voz
        e no vídeo, que é o que destrava o Go Live. Nada mais do PC passa por ele.
      </p>
    </Band>
  );
}
