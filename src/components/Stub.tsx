import { TitleBar } from "./TitleBar";
import { PHASE_TAG, PHASE_TITLE } from "../lib/format";
import { isSilent, type TunnelStatus } from "../lib/types";

interface StubProps {
  status: TunnelStatus;
  pending: boolean;
  error: string | null;
  onStart: () => void;
  onStop: () => void;
  onRelaunchElevated: () => void;
}

/**
 * O talão: em que estado a saída está e o que fazer com ela. Os detalhes
 * (servidor, convite, tráfego) ficam na faixa logo abaixo.
 */
export function Stub({ status, pending, error, onStart, onStop, onRelaunchElevated }: StubProps) {
  const up = status.phase === "up" || status.phase === "starting";
  const hasProfile = status.profile !== null;
  const silent = isSilent(status);
  const [word, rest] = hasProfile ? PHASE_TITLE[status.phase] : ["Saída", "sem convite"];

  const lead = !hasProfile
    ? "Cole o convite que você recebeu, na faixa abaixo"
    : silent
      ? "Servidor sem resposta · veja a faixa abaixo"
      : status.phase === "up"
        ? `Discord saindo por ${status.profile?.serverLabel ?? status.profile?.endpoint}`
        : status.phase === "starting"
          ? "Abrindo o driver e falando com o servidor"
          : status.phase === "failed"
            ? "A saída caiu · o motivo está abaixo"
            : `Convite de ${status.profile?.serverLabel ?? "um servidor"} guardado`;

  return (
    <>
      <TitleBar
        right={
          <span className="bar__state">
            <span className={`dot dot--${status.phase}`} aria-hidden />
            {hasProfile ? PHASE_TAG[status.phase] : "sem convite"}
          </span>
        }
      />

      <div className="stub">
        <div
          className={`stub__backdrop${status.phase === "up" && !silent ? " stub__backdrop--live" : ""}`}
          aria-hidden
        />

        <span className="stub__eyebrow">Saída do Discord</span>
        <h1 className="stub__title">
          {word} <span>{rest}</span>
        </h1>
        <span className="stub__est">Só o Discord · Nada mais do PC</span>

        <p className="stub__lead">{lead}</p>

        {(error || status.error) && (
          <p className="notice" role="alert" style={{ alignSelf: "stretch", textAlign: "left" }}>
            <strong>Não foi possível.</strong>
            {error ?? status.error}
          </p>
        )}

        {hasProfile && (
          <div className="actions">
            {up ? (
              <button type="button" onClick={onStop} disabled={pending}>
                {pending ? "Desligando…" : "Desligar"}
              </button>
            ) : status.elevated ? (
              <button
                type="button"
                className="solid"
                onClick={onStart}
                disabled={pending || status.component !== "ready"}
              >
                {pending ? "Ligando…" : "Ligar"}
              </button>
            ) : (
              <button
                type="button"
                className="solid"
                onClick={onRelaunchElevated}
                disabled={pending || status.component !== "ready"}
                title="O driver que separa o Discord do resto só abre como administrador"
              >
                Reabrir como administrador
              </button>
            )}
          </div>
        )}
      </div>
    </>
  );
}
