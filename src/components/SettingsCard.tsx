import { useEffect, useState } from "react";

import { Card } from "./Card";
import type { Config } from "../lib/types";

interface SettingsCardProps {
  config: Config;
  saving: boolean;
  error: string | null;
  onSave: (next: Config) => void;
}

type PortField = "socksPort" | "httpPort" | "controlPort";

const PORT_FIELDS: Array<{ key: PortField; label: string; hint: string }> = [
  { key: "socksPort", label: "SOCKS5", hint: "porta que os apps usam" },
  { key: "httpPort", label: "HTTP", hint: "para clientes sem suporte a SOCKS" },
  { key: "controlPort", label: "Control", hint: "uso interno do Nika" },
];

export function SettingsCard({ config, saving, error, onSave }: SettingsCardProps) {
  // Rascunho local: portas só valem quando o usuário confirma; os toggles são
  // aplicados na hora, que é o que se espera de um switch.
  const [draft, setDraft] = useState(config);
  useEffect(() => setDraft(config), [config]);

  const portsChanged = PORT_FIELDS.some(({ key }) => draft[key] !== config[key]);

  function toggle(key: "autostart" | "autoConnect", value: boolean) {
    const next = { ...draft, [key]: value };
    setDraft(next);
    onSave(next);
  }

  return (
    <Card title="Configurações">
      <div className="ports">
        {PORT_FIELDS.map(({ key, label, hint }) => (
          <label key={key}>
            <span>{label}</span>
            <input
              type="number"
              min={1024}
              max={65535}
              value={draft[key]}
              onChange={(event) =>
                setDraft({ ...draft, [key]: Number(event.target.value) })
              }
            />
            <small>{hint}</small>
          </label>
        ))}
      </div>

      {portsChanged && (
        <div className="actions">
          <button
            type="button"
            className="primary"
            disabled={saving}
            onClick={() => onSave(draft)}
          >
            {saving ? "Salvando…" : "Aplicar portas"}
          </button>
          <button type="button" className="ghost" onClick={() => setDraft(config)}>
            Descartar
          </button>
          <span className="muted">Reinicia o Tor se ele estiver no ar.</span>
        </div>
      )}

      <label className="switch">
        <input
          type="checkbox"
          checked={draft.autostart}
          onChange={(event) => toggle("autostart", event.target.checked)}
        />
        <span>Iniciar com o Windows</span>
      </label>

      <label className="switch">
        <input
          type="checkbox"
          checked={draft.autoConnect}
          onChange={(event) => toggle("autoConnect", event.target.checked)}
        />
        <span>Conectar automaticamente ao abrir</span>
      </label>

      {error && <p className="alert">{error}</p>}
    </Card>
  );
}
