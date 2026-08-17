import { useCallback, useEffect, useState } from "react";

import { Card } from "./Card";
import { api, errorMessage } from "../lib/ipc";
import { formatBytes } from "../lib/format";
import type { Circuit, ExitIp, TorStatus, Traffic } from "../lib/types";

/** GETINFO de tráfego é local e barato; 5s dá sensação de tempo real. */
const TRAFFIC_INTERVAL = 5000;

interface CircuitCardProps {
  status: TorStatus;
}

export function CircuitCard({ status }: CircuitCardProps) {
  const connected = status.phase === "connected";

  const [circuit, setCircuit] = useState<Circuit | null>(null);
  const [traffic, setTraffic] = useState<Traffic | null>(null);
  const [exitIp, setExitIp] = useState<ExitIp | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      setCircuit(await api.getCircuit());
    } catch (failure) {
      setError(errorMessage(failure));
    }
  }, []);

  // O circuito muda a cada nova identidade; recarregar ao conectar cobre o caso
  // comum sem ficar consultando à toa.
  useEffect(() => {
    if (!connected) {
      setCircuit(null);
      setTraffic(null);
      setExitIp(null);
      return;
    }

    void refresh();

    let mounted = true;
    const tick = () => {
      api
        .getTraffic()
        .then((next) => mounted && setTraffic(next))
        .catch(() => undefined);
    };

    tick();
    const timer = window.setInterval(tick, TRAFFIC_INTERVAL);

    return () => {
      mounted = false;
      window.clearInterval(timer);
    };
  }, [connected, refresh, status.attempt]);

  /** RF-15: só sob demanda — nunca automático. */
  async function checkExitIp() {
    setChecking(true);
    setError(null);
    try {
      setExitIp(await api.checkExitIp());
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setChecking(false);
    }
  }

  if (!connected) {
    return (
      <Card title="Circuito">
        <p className="muted">Disponível quando o Tor estiver conectado.</p>
      </Card>
    );
  }

  return (
    <Card
      title="Circuito"
      action={
        <button type="button" className="ghost" onClick={() => void refresh()}>
          Atualizar
        </button>
      }
    >
      {circuit ? (
        <ol className="circuit">
          {circuit.path.map((relay, index) => (
            <li key={relay.fingerprint}>
              <span className="circuit__hop">{index + 1}</span>
              <span className="circuit__name" title={relay.fingerprint}>
                {relay.nickname}
              </span>
              <span className="circuit__country">{relay.country ?? "—"}</span>
            </li>
          ))}
        </ol>
      ) : (
        <p className="muted">Nenhum circuito de uso geral construído ainda.</p>
      )}

      {traffic && (
        <p className="muted">
          Tráfego: ↓ {formatBytes(traffic.read)} · ↑ {formatBytes(traffic.written)}
        </p>
      )}

      <div className="actions">
        <button type="button" onClick={() => void checkExitIp()} disabled={checking}>
          {checking ? "Verificando…" : "Verificar IP de saída"}
        </button>
        {exitIp && (
          <span className={exitIp.isTor ? "pill pill--ok" : "pill pill--warn"}>
            {exitIp.ip} {exitIp.isTor ? "· via Tor" : "· fora do Tor!"}
          </span>
        )}
      </div>

      {error && <p className="alert">{error}</p>}
    </Card>
  );
}
