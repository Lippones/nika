import { PHASE_TAG, PHASE_TITLE } from "../lib/format";
import { isActive, type TorStatus } from "../lib/types";

/** Células do medidor de bootstrap. */
const CELLS = 28;

interface StubProps {
  status: TorStatus;
  pending: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onNewIdentity: () => void;
}

/** O talão: quem emitiu o bilhete, em que estado ele está e o que fazer com ele. */
export function Stub({
  status,
  pending,
  onConnect,
  onDisconnect,
  onNewIdentity,
}: StubProps) {
  const active = isActive(status.phase);
  const connected = status.phase === "connected";
  const showMeter = status.phase === "starting" || status.phase === "bootstrapping";
  const [word, rest] = PHASE_TITLE[status.phase];
  const filled = Math.round((status.bootstrap / 100) * CELLS);

  return (
    <>
      <header className="bar">
        <span>Nika · Proxy Tor</span>
        <span className="bar__state">
          <span className={`dot dot--${status.phase}`} aria-hidden />
          {PHASE_TAG[status.phase]}
        </span>
      </header>

      <div className="stub">
        <div
          className={`stub__backdrop${connected ? " stub__backdrop--live" : ""}`}
          aria-hidden
        />

        <span className="stub__eyebrow">Proxy Tor local</span>
        <h1 className="stub__title">
          {word} <span>{rest}</span>
        </h1>
        <span className="stub__est">Sem navegador · Sem admin</span>

        <p className="stub__lead">{status.summary}</p>

        {showMeter && (
          <div className="meter" style={{ alignSelf: "stretch" }}>
            <div className="meter__row">
              <span>Bootstrap</span>
              <span className="meter__value">{status.bootstrap}%</span>
            </div>
            <div
              className="meter__cells"
              role="progressbar"
              aria-label="Progresso do bootstrap"
              aria-valuenow={status.bootstrap}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              {Array.from({ length: CELLS }, (_, index) => (
                <span
                  key={index}
                  className={`meter__cell${index < filled ? " meter__cell--on" : ""}`}
                />
              ))}
            </div>
          </div>
        )}

        {status.error && (
          <p className="notice" style={{ alignSelf: "stretch", textAlign: "left" }}>
            <strong>Não foi possível conectar</strong>
            {status.error}
          </p>
        )}

        <div className="actions">
          {active ? (
            <button type="button" onClick={onDisconnect} disabled={pending}>
              Desconectar
            </button>
          ) : (
            <button type="button" className="solid" onClick={onConnect} disabled={pending}>
              Conectar
            </button>
          )}
          <button
            type="button"
            onClick={onNewIdentity}
            disabled={pending || !connected}
            title="Pede um novo circuito ao Tor (SIGNAL NEWNYM)"
          >
            Trocar circuito
          </button>
        </div>
      </div>
    </>
  );
}
