/**
 * Aviso permanente exigido pelo PRD (§4): fica sempre visível, não só no
 * primeiro uso.
 */
export function Disclaimer() {
  return (
    <p className="disclaimer">
      <strong>Isto troca o seu IP, não te torna anônimo.</strong> Fingerprint de
      navegador, WebRTC, telemetria de apps e contas logadas continuam te
      identificando. Para navegação anônima, use o Tor Browser.
    </p>
  );
}
