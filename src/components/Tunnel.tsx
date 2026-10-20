import { useState } from "react";

import { Band } from "./Band";
import { InviteInput } from "./InviteInput";
import { useAction } from "../hooks/useAction";
import { useInvitePreview } from "../hooks/useInvitePreview";
import { formatBytes, handshakeLabel } from "../lib/format";
import { api } from "../lib/ipc";
import { isSilent, type Config, type TunnelStatus } from "../lib/types";

interface TunnelProps {
  status: TunnelStatus;
  loading: boolean;
  config: Config;
  onSave: (next: Config) => void;
}

/**
 * RF-50 a RF-60 (docs/discord-tunnel.md). Os detalhes da saída: de quem é o
 * servidor, para quem é o convite, se ele responde, quanto passou. Ligar e
 * desligar ficam no talão, em cima. A faixa mostra o que o engine reporta,
 * não o que o app acha que fez.
 */
export function Tunnel({ status, loading, config, onSave }: TunnelProps) {
  const action = useAction();
  const [draft, setDraft] = useState("");
  const preview = useInvitePreview(draft);

  if (loading) return null;

  if (!status.supported) {
    return (
      <Band label="Convite">
        <p className="empty">disponível só no Windows</p>
      </Band>
    );
  }

  const up = status.phase === "up" || status.phase === "starting";
  const silent = isSilent(status);
  const neverAnswered =
    status.phase === "up" && status.lastHandshakeSecs === null && status.flows > 0;

  if (!status.profile) {
    return (
      <Band label="Convite">
        <InviteInput
          value={draft}
          onChange={setDraft}
          profile={preview.profile}
          error={preview.error}
          checking={preview.checking}
        />

        {action.error && (
          <p className="notice" role="alert">
            <strong>Não foi possível guardar.</strong>
            {action.error}
          </p>
        )}

        <div className="actions">
          <button
            type="button"
            className="solid"
            disabled={action.pending || preview.profile === null}
            onClick={() => void action.run(() => api.tunnelImport(draft).then(() => setDraft("")))}
          >
            {action.pending ? "Guardando…" : "Usar convite"}
          </button>
        </div>

        <p className="invite__hint">
          Para ter o seu próprio servidor e convidar outras pessoas, veja{" "}
          <code>docs/oracle-exit.md</code> no repositório.
        </p>
      </Band>
    );
  }

  return (
    <Band label="Convite">
      <ul className="rows">
        <li className="row">
          <span className="row__key">Servidor</span>
          <span className="row__value row__value--strong">
            {status.profile.serverLabel ?? "sem rótulo"}
          </span>
        </li>
        <li className="row">
          <span className="row__key">Endereço</span>
          <span className="row__value">{status.profile.endpoint}</span>
        </li>
        {status.profile.peerName && (
          <li className="row">
            <span className="row__key">Para</span>
            <span className="row__value">{status.profile.peerName}</span>
          </li>
        )}
        <li className="row">
          <span className="row__key">Handshake</span>
          <span className="row__value">{handshakeLabel(status)}</span>
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

      {(silent || neverAnswered) && (
        <p className="notice" role="status">
          <strong>Sem resposta do servidor.</strong>
          Ele pode estar desligado, ou o seu convite foi revogado. O Discord não
          conecta enquanto isso. Fale com quem te convidou.
        </p>
      )}

      {action.error && (
        <p className="notice" role="alert">
          <strong>Não foi possível.</strong>
          {action.error}
        </p>
      )}

      {status.component !== "ready" && (
        <p className="notice">
          <strong>Componente ausente.</strong>
          Este build do Nika não inclui o driver WinDivert. Rode{" "}
          <code>scripts/fetch-windivert.ps1</code> e gere o instalador de novo.
        </p>
      )}

      {!status.elevated && status.component === "ready" && !up && (
        <p className="notice">
          <strong>Precisa de administrador.</strong>
          Separar o tráfego do Discord do resto do PC exige um driver, e ele só abre
          com permissão de administrador. O Nika reabre pedindo (UAC) e liga em seguida.
        </p>
      )}

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

      {!up && (
        <div className="actions">
          <button
            type="button"
            className="ghost"
            disabled={action.pending}
            onClick={() => void action.run(api.tunnelForget)}
          >
            Esquecer convite
          </button>
        </div>
      )}
    </Band>
  );
}
