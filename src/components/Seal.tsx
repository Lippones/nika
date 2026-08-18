/**
 * Carimbo do bilhete. O aviso obrigatório do PRD (§4) merecia mais que uma
 * linha de rodapé: aqui ele é o selo, com o texto correndo na borda e o
 * resumo no centro.
 */
export function Seal() {
  return (
    <svg className="seal" viewBox="0 0 100 100" aria-hidden focusable="false">
      <defs>
        <path
          id="seal-ring"
          d="M 50 50 m -37 0 a 37 37 0 1 1 74 0 a 37 37 0 1 1 -74 0"
        />
      </defs>
      <circle cx="50" cy="50" r="47.5" fill="none" stroke="currentColor" strokeWidth="1" />
      <circle cx="50" cy="50" r="29" fill="none" stroke="currentColor" strokeWidth="0.6" />
      <g className="seal__ring">
        <text fontSize="8.4" letterSpacing="1.6" fill="currentColor">
          <textPath href="#seal-ring" startOffset="0">
            TROCA DE IP · NÃO É ANONIMATO · TROCA DE IP · NÃO É ANONIMATO ·
          </textPath>
        </text>
      </g>
      <text
        x="50"
        y="53.5"
        textAnchor="middle"
        fontSize="13"
        fontWeight="600"
        letterSpacing="0.5"
        fill="currentColor"
      >
        IP ≠ ID
      </text>
    </svg>
  );
}
