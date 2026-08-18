import { useState } from "react";

import { Circuit } from "./components/Circuit";
import { Disclaimer } from "./components/Disclaimer";
import { DiscordProxy } from "./components/DiscordProxy";
import { Endpoints } from "./components/Endpoints";
import { LogPanel } from "./components/LogPanel";
import { Onboarding } from "./components/Onboarding";
import { Preferences } from "./components/Preferences";
import { Stub } from "./components/Stub";
import { useAction } from "./hooks/useAction";
import { useConfig } from "./hooks/useConfig";
import { useDiscord } from "./hooks/useDiscord";
import { useTorStatus } from "./hooks/useTorStatus";
import { api } from "./lib/ipc";

export default function App() {
  const status = useTorStatus();
  const { config, save, saving, error: configError } = useConfig();
  const action = useAction();
  const discord = useDiscord();
  const [identityNote, setIdentityNote] = useState(false);

  async function newIdentity() {
    setIdentityNote(false);
    await action.run(api.newIdentity);
    setIdentityNote(true);
  }

  // Enquanto a config não carrega, nada de janela: um flash da tela principal
  // seguido do onboarding seria pior que meio segundo de vazio.
  if (!config) return <div className="stage" aria-busy="true" />;

  if (!config.onboarded) {
    return (
      <Onboarding
        status={status}
        discord={discord.status}
        discordLoading={discord.loading}
        config={config}
        onFinish={() => void save({ ...config, onboarded: true })}
      />
    );
  }

  return (
    <div className="stage">
      <main className="ticket">
        <Stub
          status={status}
          pending={action.pending}
          onConnect={() => void action.run(api.connect)}
          onDisconnect={() => void action.run(api.disconnect)}
          onNewIdentity={() => void newIdentity()}
        />

        <div className="perf" />

        {(action.error || (identityNote && !action.error)) && (
          <section className="band">
            {action.error ? (
              <p className="notice">
                <strong>A ação não foi concluída</strong>
                {action.error}
              </p>
            ) : (
              <p className="notice">
                <strong>Circuito novo pedido</strong>
                Conexões já abertas seguem no circuito antigo até serem refeitas.
              </p>
            )}
          </section>
        )}

        <Endpoints config={config} />

        <DiscordProxy
          status={discord.status}
          loading={discord.loading}
          connected={status.phase === "connected"}
          config={config}
          onSave={(next) => void save(next)}
        />

        <Circuit status={status} />

        <Preferences
          config={config}
          saving={saving}
          error={configError}
          onSave={(next) => void save(next)}
        />

        <LogPanel />

        <Disclaimer />

        <footer className="foot">
          <span className="foot__note">Fechar a janela só esconde o Nika na bandeja</span>
          <button type="button" className="ghost" onClick={() => void api.quit()}>
            Sair
          </button>
        </footer>
      </main>
    </div>
  );
}
