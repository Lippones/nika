import { useState } from "react";

import { CircuitCard } from "./components/CircuitCard";
import { Disclaimer } from "./components/Disclaimer";
import { LogCard } from "./components/LogCard";
import { ProxyCard } from "./components/ProxyCard";
import { SettingsCard } from "./components/SettingsCard";
import { StatusCard } from "./components/StatusCard";
import { useAction } from "./hooks/useAction";
import { useConfig } from "./hooks/useConfig";
import { useTorStatus } from "./hooks/useTorStatus";
import { api } from "./lib/ipc";

export default function App() {
  const status = useTorStatus();
  const { config, save, saving, error: configError } = useConfig();
  const action = useAction();
  const [identityNote, setIdentityNote] = useState(false);

  async function newIdentity() {
    setIdentityNote(false);
    await action.run(api.newIdentity);
    setIdentityNote(true);
  }

  return (
    <main className="app">
      <StatusCard
        status={status}
        pending={action.pending}
        onConnect={() => void action.run(api.connect)}
        onDisconnect={() => void action.run(api.disconnect)}
        onNewIdentity={() => void newIdentity()}
      />

      {action.error && <p className="alert">{action.error}</p>}

      {identityNote && !action.error && (
        <p className="note">
          Novo circuito pedido. Conexões já abertas continuam no circuito antigo
          até serem refeitas.
        </p>
      )}

      <Disclaimer />

      {config && <ProxyCard config={config} />}

      <CircuitCard status={status} />

      {config && (
        <SettingsCard
          config={config}
          saving={saving}
          error={configError}
          onSave={(next) => void save(next)}
        />
      )}

      <LogCard />

      <footer className="footer">
        <span>Fechar a janela apenas esconde o app na bandeja.</span>
        <button type="button" className="ghost" onClick={() => void api.quit()}>
          Sair do Nika
        </button>
      </footer>
    </main>
  );
}
