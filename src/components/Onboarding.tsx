import { useState } from "react";

import { InviteInput } from "./InviteInput";
import { Seal } from "./Seal";
import { TitleBar } from "./TitleBar";
import { useInvitePreview } from "../hooks/useInvitePreview";
import { api, errorMessage } from "../lib/ipc";
import type { TunnelStatus } from "../lib/types";
import cloud from "../assets/cloud.png";

interface OnboardingProps {
  tunnel: TunnelStatus;
  /**
   * Marca `onboarded` e leva à janela principal. Devolve a promise da gravação
   * porque o caminho do convite reabre o app elevado logo depois: a instância
   * nova precisa encontrar a config já salva.
   */
  onFinish: () => Promise<void> | void;
}

/** O que muda ao usar um convite (docs/discord-tunnel.md §4). */
const FACTS = [
  {
    key: "Só Discord",
    text: "Voz, vídeo e mensagens do Discord saem pelo servidor de quem te convidou. Nada mais do PC passa por ele.",
  },
  {
    key: "Admin",
    text: "Separar o Discord do resto exige um driver. O Nika reabre pedindo permissão (UAC) uma vez.",
  },
  {
    key: "IP",
    text: "O Discord passa a ver o IP do servidor, não o seu. Isso destrava o Go Live. Não é anonimato.",
  },
];

/**
 * Primeira execução, em duas telas do mesmo bilhete: boas-vindas (com o selo
 * do aviso) e o convite. Quem já tem o convite na mão chega ao Discord
 * funcionando em dois cliques e um UAC.
 */
export function Onboarding({ tunnel, onFinish }: OnboardingProps) {
  // `?step=invite` e `?invite=<texto>` existem só para o preview web
  // (`npm run dev`); dentro do Tauri não há query string.
  const params =
    typeof location !== "undefined" ? new URLSearchParams(location.search) : null;
  const [step, setStep] = useState<"welcome" | "invite">(
    params?.get("step") === "invite" ? "invite" : "welcome",
  );
  const [invite, setInvite] = useState(params?.get("invite") ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const preview = useInvitePreview(invite);

  /**
   * Guarda o convite, conclui o onboarding e liga. Sem elevação, "ligar" é
   * reabrir como administrador: este processo sai e o novo sobe com `--tunnel`.
   */
  async function useInvite() {
    setBusy(true);
    setError(null);
    try {
      await api.tunnelImport(invite);
      await onFinish();
      if (tunnel.elevated) {
        await api.tunnelStart();
      } else {
        await api.relaunchElevated();
      }
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  if (step === "welcome") {
    return (
      <div className="stage">
        <main className="ticket">
          <TitleBar right={<span className="onb__step">Passo 1 de 2</span>} />

          <div className="stub onb">
            <div className="stub__backdrop" aria-hidden />
            <img className="onb__mark" src={cloud} alt="" />
            <span className="stub__eyebrow">Instalado</span>
            <h1 className="stub__title">
              Discord <span>sem bloqueio</span>
            </h1>
            <p className="onb__lead">
              Alguém com servidor te mandou um convite? Com ele, o Go Live, a voz
              e o vídeo do Discord voltam. Só o Discord muda de caminho.
            </p>
          </div>

          <div className="perf" />

          <section className="stamp">
            <Seal />
            <p className="stamp__text">
              <strong>Isto troca o IP do Discord. Não te torna anônimo.</strong>
              O Discord passa a ver o IP do servidor de quem te convidou; a sua
              conta e o seu PC continuam os mesmos.
            </p>
          </section>

          <div className="onb__actions">
            <button
              type="button"
              className="solid"
              onClick={() => setStep("invite")}
              disabled={!tunnel.supported}
            >
              Tenho um convite
            </button>
            <button type="button" className="ghost" onClick={() => void onFinish()}>
              Ainda não
            </button>
          </div>
        </main>
      </div>
    );
  }

  const ready = preview.profile !== null && tunnel.component === "ready";
  const primaryLabel = busy
    ? "Guardando convite…"
    : tunnel.component !== "ready"
      ? "Componente ausente"
      : tunnel.elevated
        ? "Usar convite e ligar"
        : "Usar convite e reabrir como admin";

  return (
    <div className="stage">
      <main className="ticket">
        <TitleBar right={<span className="onb__step">Passo 2 de 2</span>} />

        <div className="stub onb" style={{ minHeight: 200 }}>
          <div className="stub__backdrop" aria-hidden />
          <img className="onb__mark" src={cloud} alt="" style={{ width: 112, opacity: 0.7 }} />
          <span className="stub__eyebrow">Passo 2 de 2 · Convite</span>
          <h1 className="stub__title">
            Entrar pelo <span>servidor de alguém</span>
          </h1>
          <span className="stub__est">
            {preview.profile
              ? `Convite lido · servidor ${preview.profile.serverLabel ?? preview.profile.endpoint}`
              : "Cole o convite que você recebeu"}
          </span>
        </div>

        <div className="perf" />

        <section className="band">
          <InviteInput
            value={invite}
            onChange={setInvite}
            profile={preview.profile}
            error={preview.error}
            checking={preview.checking}
          />
        </section>

        <section className="band">
          <ul className="facts">
            {FACTS.map((fact) => (
              <li key={fact.key} className="fact">
                <span className="fact__key">{fact.key}</span>
                <span className="fact__text">{fact.text}</span>
              </li>
            ))}
          </ul>
        </section>

        {tunnel.component !== "ready" && (
          <section className="band">
            <p className="notice">
              <strong>Componente ausente.</strong>
              Este build do Nika não inclui o driver WinDivert, então o convite não
              pode ser usado aqui. Peça a quem te convidou o instalador completo.
            </p>
          </section>
        )}

        {error && (
          <section className="band">
            <p className="notice" role="alert">
              <strong>Não deu para usar o convite.</strong>
              {error}
            </p>
          </section>
        )}

        <div className="onb__actions">
          <button
            type="button"
            className="solid"
            disabled={!ready || busy}
            onClick={() => void useInvite()}
          >
            {primaryLabel}
          </button>
          <button type="button" className="ghost" onClick={() => setStep("welcome")}>
            Voltar
          </button>
        </div>
      </main>
    </div>
  );
}
