import type { TunnelProfile } from "../lib/types";

interface InviteInputProps {
  value: string;
  onChange: (next: string) => void;
  profile: TunnelProfile | null;
  error: string | null;
  checking: boolean;
}

/** Quem te convidou e para onde o Discord vai sair: a leitura do convite. */
export function InviteSummary({ profile }: { profile: TunnelProfile }) {
  return (
    <ul className="rows">
      <li className="row">
        <span className="row__key">Convite</span>
        <span className="row__value row__value--strong">
          {profile.peerName ? `para ${profile.peerName}` : "sem nome"}
        </span>
      </li>
      <li className="row">
        <span className="row__key">Servidor</span>
        <span className="row__value">
          {profile.serverLabel ?? "sem rótulo"} · {profile.endpoint}
        </span>
      </li>
    </ul>
  );
}

/**
 * O campo onde o convite é colado, com a leitura ao vivo logo abaixo. Usado no
 * onboarding e na faixa, para o gesto ser o mesmo nos dois lugares.
 */
export function InviteInput({ value, onChange, profile, error, checking }: InviteInputProps) {
  const state = value.trim().length === 0 ? "" : profile ? " paste--ok" : error ? " paste--bad" : "";

  return (
    <>
      <textarea
        className={`paste${state}`}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        aria-label="Convite"
        aria-invalid={error !== null}
        placeholder="nika://convite/…"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />

      {profile ? (
        <InviteSummary profile={profile} />
      ) : error && !checking ? (
        <p className="notice" role="alert">
          <strong>Convite não reconhecido.</strong>
          {error} Copie de novo a linha inteira que você recebeu, do{" "}
          <code>nika://</code> até o fim.
        </p>
      ) : (
        <p className="invite__hint">
          Cole a linha <code>nika://convite/…</code> que você recebeu, ou o arquivo{" "}
          <code>.conf</code> inteiro. O convite é só seu: não repasse.
        </p>
      )}
    </>
  );
}
