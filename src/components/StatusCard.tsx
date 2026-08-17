import { PHASE_LABEL } from "../lib/format";
import { isActive, type TorStatus } from "../lib/types";

interface StatusCardProps {
  status: TorStatus;
  pending: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onNewIdentity: () => void;
}

export function StatusCard({
  status,
  pending,
  onConnect,
  onDisconnect,
  onNewIdentity,
}: StatusCardProps) {
  const active = isActive(status.phase);
  const showProgress = status.phase === "starting" || status.phase === "bootstrapping";

  return (
    <section className="card status">
      <div className="status__head">
        <span className={`dot dot--${status.phase}`} aria-hidden />
        <div>
          <h1>{PHASE_LABEL[status.phase]}</h1>
          <p className="status__summary">{status.summary}</p>
        </div>
        {status.phase === "connected" && <span className="badge">100%</span>}
      </div>

      {showProgress && (
        <div
          className="progress"
          role="progressbar"
          aria-valuenow={status.bootstrap}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <div className="progress__bar" style={{ width: `${status.bootstrap}%` }} />
          <span className="progress__label">{status.bootstrap}%</span>
        </div>
      )}

      {status.error && <p className="alert">{status.error}</p>}

      <div className="actions">
        {active ? (
          <button type="button" onClick={onDisconnect} disabled={pending}>
            Desconectar
          </button>
        ) : (
          <button type="button" className="primary" onClick={onConnect} disabled={pending}>
            Conectar
          </button>
        )}
        <button
          type="button"
          onClick={onNewIdentity}
          disabled={pending || status.phase !== "connected"}
          title="Pede um novo circuito ao Tor (SIGNAL NEWNYM)"
        >
          Nova identidade
        </button>
      </div>
    </section>
  );
}
