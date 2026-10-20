import { Band } from "./Band";
import type { Config } from "../lib/types";

interface PreferencesProps {
  config: Config;
  saving: boolean;
  error: string | null;
  onSave: (next: Config) => void;
}

export function Preferences({ config, saving, error, onSave }: PreferencesProps) {
  return (
    <Band label="Preferências">
      <div className="switches">
        <label className="switch">
          <span>Iniciar com o Windows</span>
          <input
            type="checkbox"
            checked={config.autostart}
            disabled={saving}
            onChange={(event) => onSave({ ...config, autostart: event.target.checked })}
          />
        </label>
      </div>

      <p className="invite__hint">
        Ao iniciar com o Windows o Nika nasce na bandeja, sem administrador. A
        saída só liga sozinha se "Ligar ao abrir" estiver marcado e o Nika for
        aberto como administrador.
      </p>

      {error && (
        <p className="notice" role="alert">
          <strong>As preferências não foram salvas.</strong>
          {error}
        </p>
      )}
    </Band>
  );
}
