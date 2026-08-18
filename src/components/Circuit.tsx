import { useCallback, useEffect, useState } from "react";

import { Band } from "./Band";
import { Barcode } from "./Barcode";
import { api, errorMessage } from "../lib/ipc";
import { formatBytes, hopRole } from "../lib/format";
import type { Circuit as CircuitData, ExitIp, TorStatus, Traffic } from "../lib/types";

/** GETINFO de tráfego é local e barato; 5s dá sensação de tempo real. */
const TRAFFIC_INTERVAL = 5000;

interface CircuitProps {
  status: TorStatus;
}

export function Circuit({ status }: CircuitProps) {
  const connected = status.phase === "connected";

  const [circuit, setCircuit] = useState<CircuitData | null>(null);
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
      <Band label="Circuito">
        <p className="empty">O caminho aparece aqui quando o Tor conectar</p>
      </Band>
    );
  }

  const exit = circuit?.path.at(-1);

  return (
    <Band
      label="Circuito"
      action={
        <button type="button" className="ghost" onClick={() => void refresh()}>
          Atualizar
        </button>
      }
    >
      {circuit ? (
        <ol className="hops">
          {circuit.path.map((relay, index) => {
            const role = hopRole(index, circuit.path.length);
            return (
              <li
                key={relay.fingerprint}
                className={`hop${role === "saída" ? " hop--exit" : ""}`}
              >
                <span className="hop__role">{role}</span>
                <span className="hop__name" title={relay.fingerprint}>
                  {relay.nickname}
                </span>
                <span className="hop__cc">{relay.country ?? "--"}</span>
              </li>
            );
          })}
        </ol>
      ) : (
        <p className="empty">Nenhum circuito de uso geral construído ainda</p>
      )}

      {exit && (
        <ul className="rows">
          <li className="row">
            <span className="row__key">Fingerprint</span>
            <Barcode source={exit.fingerprint} strong />
          </li>
        </ul>
      )}

      {traffic && (
        <div className="traffic">
          <div className="traffic__item">
            <span className="traffic__value">{formatBytes(traffic.read)}</span>
            <span className="label">Recebido</span>
          </div>
          <div className="traffic__item">
            <span className="traffic__value">{formatBytes(traffic.written)}</span>
            <span className="label">Enviado</span>
          </div>
        </div>
      )}

      <div className="actions">
        <button type="button" onClick={() => void checkExitIp()} disabled={checking}>
          {checking ? "Verificando…" : "Verificar IP de saída"}
        </button>
        {exitIp && (
          <span className="verdict">
            <code className="row__value--strong">{exitIp.ip}</code>
            <span className={`verdict__tag${exitIp.isTor ? "" : " verdict__tag--alert"}`}>
              {exitIp.isTor ? "via Tor" : "fora do Tor"}
            </span>
          </span>
        )}
      </div>

      {error && (
        <p className="notice">
          <strong>A consulta falhou</strong>
          {error}
        </p>
      )}
    </Band>
  );
}
