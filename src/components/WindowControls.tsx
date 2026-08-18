import { api } from "../lib/ipc";

/**
 * A janela não tem moldura nativa (`decorations: false`): estes são os controles
 * dela. Minimizar e fechar passam por comandos do core; fechar é esconder na
 * bandeja, o mesmo que o X de antes fazia. Ficam fora da região de arrasto para
 * continuarem clicáveis.
 */
export function WindowControls() {
  return (
    <div className="wctl">
      <button
        type="button"
        className="wctl__btn"
        title="Minimizar"
        aria-label="Minimizar"
        onClick={() => void api.minimizeWindow()}
      >
        <svg viewBox="0 0 12 12" aria-hidden focusable="false">
          <line x1="2.5" y1="6" x2="9.5" y2="6" />
        </svg>
      </button>
      <button
        type="button"
        className="wctl__btn"
        title="Esconder na bandeja"
        aria-label="Esconder na bandeja"
        onClick={() => void api.hideWindow()}
      >
        <svg viewBox="0 0 12 12" aria-hidden focusable="false">
          <line x1="3" y1="3" x2="9" y2="9" />
          <line x1="9" y1="3" x2="3" y2="9" />
        </svg>
      </button>
    </div>
  );
}
