import { useState } from "react";

import { InviteInput } from "./InviteInput";
import { Seal } from "./Seal";
import { TitleBar } from "./TitleBar";
import { useInvitePreview } from "../hooks/useInvitePreview";
import { api, errorMessage } from "../lib/ipc";
import { PHASE_TAG } from "../lib/format";
import {
  isActive,
  type Config,
  type DiscordStatus,
  type TorStatus,
  type TunnelStatus,
} from "../lib/types";
import cloud from "../assets/cloud.png";

interface OnboardingProps {
  status: TorStatus;
  discord: DiscordStatus;
  /** Uma varredura ainda não voltou — evita piscar "não encontrei o Discord". */
  discordLoading: boolean;
  tunnel: TunnelStatus;
  config: Config;
  /**
   * Marca `onboarded` e leva à janela principal. Devolve a promise da gravação
   * porque o caminho do convite reabre o app elevado logo depois: a instância
   * nova precisa encontrar a config já salva.
   */
  onFinish: () => Promise<void> | void;
}

/** O que muda ao usar um convite (docs/discord-tunnel.md §4). */
const INVITE_FACTS = [
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

/** Os avisos honestos da spec do Discord (docs/discord-dll.md §2, R-3). */
const FACTS = [
  {
    key: "Antes",
    text: "IP de saída Tor recebe captcha, verificação por telefone e, às vezes, bloqueio de conta.",
  },
  {
    key: "Voz",
    text: "Chamadas de voz saem pelo seu IP real. O Tor não transporta voz.",
  },
  {
    key: "Como",
    text: "Um componente ao lado do Discord, reaplicado quando ele atualiza. Some ao remover.",
  },
];

/**
 * Primeira execução (RF-? / spec do onboarding v2 — ver docs/onboarding-v2.html).
 * Explicação curtíssima e o install do Discord como destino; os endereços do
 * proxy ficam na janela principal, para quem aponta apps à mão.
 *
 * Duas telas do mesmo bilhete: boas-vindas (com o selo do aviso, PRD §4) e o
 * Discord. O Tor conecta dobrado no gesto de ir para o Discord — a espera é
 * mascarada pela leitura dos avisos, e o install libera quando o proxy sobe.
 */
export function Onboarding({
  status,
  discord,
  discordLoading,
  tunnel,
  config,
  onFinish,
}: OnboardingProps) {
  // `?step=invite` existe só para o preview web (`npm run dev`) abrir direto no
  // passo do convite; dentro do Tauri não há query string.
  const [step, setStep] = useState<"welcome" | "invite" | "discord">(() =>
    typeof location !== "undefined" &&
    new URLSearchParams(location.search).get("step") === "invite"
      ? "invite"
      : "welcome",
  );
  const [closeDiscord, setCloseDiscord] = useState(config.discord.allowClose);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // `?invite=<texto>` idem: pré-preenche o campo no preview web.
  const [invite, setInvite] = useState(() =>
    typeof location !== "undefined"
      ? (new URLSearchParams(location.search).get("invite") ?? "")
      : "",
  );
  const preview = useInvitePreview(invite);

  const connected = status.phase === "connected";

  /**
   * Guarda o convite, conclui o onboarding e liga. Sem elevação, "ligar" é
   * reabrir como administrador: este processo sai e o novo sobe com `--tunnel`.
   */
  async function useInvite() {
    setInstalling(true);
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
      setInstalling(false);
    }
  }

  /** Ir ao Discord já conecta o Tor: a espera roda enquanto a pessoa lê. */
  function goToDiscord() {
    if (!isActive(status.phase)) void api.connect().catch(() => undefined);
    setStep("discord");
  }

  async function install() {
    setInstalling(true);
    setError(null);
    try {
      // torHttp é o caminho recomendado (docs/discord-proxy.md D-03). O usuário
      // troca para SOCKS5 depois, na janela principal, se precisar.
      await api.discordInstall("torHttp", closeDiscord, closeDiscord);
      onFinish();
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setInstalling(false);
    }
  }

  if (step === "welcome") {
    return (
      <div className="stage">
        <main className="ticket">
          <TitleBar right={<span className="onb__step">Passo 1 de 2</span>} />

          <div className="stub onb">
            <div
              className={`stub__backdrop${connected ? " stub__backdrop--live" : ""}`}
              aria-hidden
            />
            <img className="onb__mark" src={cloud} alt="" />
            <span className="stub__eyebrow">Instalado</span>
            <h1 className="stub__title">
              Discord <span>sem bloqueio</span>
            </h1>
            <p className="onb__lead">
              Recebeu um convite de alguém com servidor? Ele traz o Go Live de volta.
              Sem convite, o Nika leva o Discord pela rede Tor.
            </p>
          </div>

          <div className="perf" />

          <section className="stamp">
            <Seal />
            <p className="stamp__text">
              <strong>Isto troca o seu IP. Não te torna anônimo.</strong>
              Fingerprint de navegador, WebRTC, telemetria de apps e contas
              logadas continuam te identificando.
            </p>
          </section>

          <div className="onb__actions">
            {tunnel.supported ? (
              <>
                <button type="button" className="solid" onClick={() => setStep("invite")}>
                  Tenho um convite
                </button>
                <button type="button" onClick={goToDiscord}>
                  Discord via Tor
                </button>
              </>
            ) : (
              <button type="button" className="solid" onClick={goToDiscord}>
                Configurar o Discord
              </button>
            )}
            <button type="button" className="ghost" onClick={() => void onFinish()}>
              Só o proxy
            </button>
          </div>
        </main>
      </div>
    );
  }

  if (step === "invite") {
    const ready = preview.profile !== null && tunnel.component === "ready";
    const primaryLabel = installing
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
              {INVITE_FACTS.map((fact) => (
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
                Este build do Nika não inclui o driver WinDivert, então o convite
                não pode ser usado aqui. Peça a quem te convidou o instalador completo.
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
              disabled={!ready || installing}
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

  // step === "discord"
  const foundDiscord = discord.installs.length > 0;
  const hasComponent = discord.component === "ready";
  const folders = discord.installs.reduce((n, i) => n + i.appDirs.length, 0);

  const est = discordLoading
    ? "Procurando o Discord…"
    : foundDiscord
      ? `Encontrado em ${folders} pasta(s)`
      : "Discord não encontrado neste usuário";

  const gateValue = connected
    ? "No ar · pronto para instalar"
    : isActive(status.phase)
      ? `Conectando · ${status.bootstrap}%`
      : "Parado";

  // A ação principal muda de papel conforme o que falta: reconectar o Tor,
  // esperar o bootstrap, ou finalmente instalar.
  let primaryLabel = "Instalar no Discord";
  let primaryDisabled = installing;
  let onPrimary: () => void = install;

  if (!connected) {
    if (isActive(status.phase)) {
      primaryLabel = "Conectando ao Tor…";
      primaryDisabled = true;
    } else {
      primaryLabel = "Conectar o Tor";
      primaryDisabled = false;
      onPrimary = () => void api.connect().catch(() => undefined);
    }
  } else if (!foundDiscord) {
    primaryLabel = "Discord não encontrado";
    primaryDisabled = true;
  } else if (!hasComponent) {
    primaryLabel = "Componente ausente";
    primaryDisabled = true;
  } else if (installing) {
    primaryLabel = "Instalando…";
  }

  return (
    <div className="stage">
      <main className="ticket">
        <TitleBar
          right={
            <span className="bar__state">
              <span className={`dot dot--${status.phase}`} aria-hidden />
              {PHASE_TAG[status.phase]}
            </span>
          }
        />

        <div className="stub onb" style={{ minHeight: 224 }}>
          <div
            className={`stub__backdrop${connected ? " stub__backdrop--live" : ""}`}
            aria-hidden
          />
          <img
            className="onb__mark"
            src={cloud}
            alt=""
            style={{ width: 124, opacity: connected ? 0.92 : 0.5 }}
          />
          <span className="stub__eyebrow">Passo 2 de 2 · Discord</span>
          <h1 className="stub__title">
            Rotear o <span>Discord</span>
          </h1>
          <span className="stub__est">{est}</span>
        </div>

        <div className="perf" />

        <section className="band">
          <div className="gate">
            <span className="gate__label">Proxy Tor</span>
            <span className="gate__value">
              <span className={`dot dot--${status.phase}`} aria-hidden />
              {gateValue}
            </span>
          </div>
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

        {!discordLoading && foundDiscord && !hasComponent && (
          <section className="band">
            <p className="notice">
              <strong>Componente ausente.</strong>
              Este build do Nika não inclui o <code>version.dll</code> do proxy do
              Discord. Você ainda pode usar o proxy com outros apps.
            </p>
          </section>
        )}

        {error && (
          <section className="band">
            <p className="notice">
              <strong>A instalação não foi concluída.</strong>
              {error}
            </p>
          </section>
        )}

        {discord.running && (
          <section className="band">
            <div className="switches">
              <label className="switch">
                <span>Fechar e reabrir o Discord agora</span>
                <input
                  type="checkbox"
                  checked={closeDiscord}
                  onChange={(event) => setCloseDiscord(event.target.checked)}
                />
              </label>
            </div>
          </section>
        )}

        <div className="onb__actions">
          <button
            type="button"
            className="solid"
            disabled={primaryDisabled}
            onClick={onPrimary}
          >
            {primaryLabel}
          </button>
          <button type="button" className="ghost" onClick={() => void onFinish()}>
            Agora não
          </button>
        </div>
      </main>
    </div>
  );
}
