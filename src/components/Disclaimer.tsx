import { Seal } from "./Seal";

/**
 * Aviso permanente exigido pelo PRD (§4): fica sempre visível, não só no
 * primeiro uso. Aqui ele é o carimbo do bilhete — presente e assinado, sem
 * disputar a leitura com o estado.
 */
export function Disclaimer() {
  return (
    <section className="stamp">
      <Seal />
      <p className="stamp__text">
        <strong>Isto troca o seu IP. Não te torna anônimo.</strong>
        Fingerprint de navegador, WebRTC, telemetria de apps e contas logadas
        continuam te identificando. Para navegação anônima, use o Tor Browser.
      </p>
    </section>
  );
}
