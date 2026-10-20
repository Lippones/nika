import { Disclaimer } from "./components/Disclaimer";
import { LogPanel } from "./components/LogPanel";
import { Onboarding } from "./components/Onboarding";
import { Preferences } from "./components/Preferences";
import { Stub } from "./components/Stub";
import { Tunnel } from "./components/Tunnel";
import { useAction } from "./hooks/useAction";
import { useConfig } from "./hooks/useConfig";
import { useTunnel } from "./hooks/useTunnel";
import { api } from "./lib/ipc";

export default function App() {
  const { config, save, saving, error: configError } = useConfig();
  const tunnel = useTunnel();
  const action = useAction();

  // Enquanto a config não carrega, nada de janela: um flash da tela principal
  // seguido do onboarding seria pior que meio segundo de vazio.
  if (!config) return <div className="stage" aria-busy="true" />;

  if (!config.onboarded) {
    return (
      <Onboarding tunnel={tunnel.status} onFinish={() => save({ ...config, onboarded: true })} />
    );
  }

  return (
    <div className="stage">
      <main className="ticket">
        <Stub
          status={tunnel.status}
          pending={action.pending}
          error={action.error}
          onStart={() => void action.run(api.tunnelStart)}
          onStop={() => void action.run(api.tunnelStop)}
          onRelaunchElevated={() => void action.run(api.relaunchElevated)}
        />

        <div className="perf" />

        <Tunnel
          status={tunnel.status}
          loading={tunnel.loading}
          config={config}
          onSave={(next) => void save(next)}
        />

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
