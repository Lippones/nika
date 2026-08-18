import { useEffect, useState } from "react";

import { Band } from "./Band";
import type { Config } from "../lib/types";

interface PreferencesProps {
  config: Config;
  saving: boolean;
  error: string | null;
  onSave: (next: Config) => void;
}

type PortField = "socksPort" | "httpPort" | "controlPort";

const PORT_FIELDS: Array<{ key: PortField; label: string; hint: string }> = [
  { key: "socksPort", label: "SOCKS5", hint: "apps apontam aqui" },
  { key: "httpPort", label: "HTTP", hint: "clientes sem socks" },
  { key: "controlPort", label: "Control", hint: "uso interno" },
];

export function Preferences({ config, saving, error, onSave }: PreferencesProps) {
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
    <Band label="Preferências">
      <div className="ports">
        {PORT_FIELDS.map(({ key, label, hint }) => (
          <label key={key} className="port">
            <span className="port__label">{label}</span>
            <input
              type="number"
              min={1024}
              max={65535}
              value={draft[key]}
              onChange={(event) =>
                setDraft({ ...draft, [key]: Number(event.target.value) })
              }
            />
            <span className="port__hint">{hint}</span>
          </label>
        ))}
      </div>

      {portsChanged && (
        <div className="actions">
          <button
            type="button"
            className="solid"
            disabled={saving}
            onClick={() => onSave(draft)}
          >
            {saving ? "Salvando…" : "Salvar portas"}
          </button>
          <button type="button" className="ghost" onClick={() => setDraft(config)}>
            Descartar
          </button>
          <span className="port__hint">Reinicia o Tor se ele estiver no ar</span>
        </div>
      )}

      <div className="switches">
        <label className="switch">
          <span>Iniciar com o Windows</span>
          <input
            type="checkbox"
            checked={draft.autostart}
            onChange={(event) => toggle("autostart", event.target.checked)}
          />
        </label>

        <label className="switch">
          <span>Conectar ao abrir</span>
          <input
            type="checkbox"
            checked={draft.autoConnect}
            onChange={(event) => toggle("autoConnect", event.target.checked)}
          />
        </label>
      </div>

      {error && (
        <p className="notice">
          <strong>As preferências não foram salvas</strong>
          {error}
        </p>
      )}
    </Band>
  );
}
