import { Seal } from "./Seal";

/**
 * Aviso permanente: fica sempre visível, não só no primeiro uso. Aqui ele é o
 * carimbo do bilhete — presente e assinado, sem disputar a leitura com o estado.
 */
export function Disclaimer() {
  return (
    <section className="stamp">
      <Seal />
      <p className="stamp__text">
        <strong>Isto troca o IP do Discord. Não te torna anônimo.</strong>
        O Discord passa a ver o IP do servidor de quem te convidou; a sua conta,
        o seu PC e o resto da sua rede continuam os mesmos.
      </p>
    </section>
  );
}
