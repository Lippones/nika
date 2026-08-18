import type { ReactNode } from "react";

import { WindowControls } from "./WindowControls";

interface TitleBarProps {
  /** Conteúdo à direita antes dos controles: o estado, ou o passo do onboarding. */
  right?: ReactNode;
}

/**
 * Barra do topo do bilhete. Também é a moldura da janela: a faixa inteira é
 * região de arrasto (`data-tauri-drag-region`) e carrega os controles de
 * minimizar/fechar. Os textos têm `pointer-events: none` no CSS para que o
 * clique caia na região de arrasto; só os botões continuam clicáveis.
 */
export function TitleBar({ right }: TitleBarProps) {
  return (
    <header className="bar" data-tauri-drag-region>
      <span className="bar__brand">Nika · Proxy Tor</span>
      <div className="bar__tools">
        {right}
        <WindowControls />
      </div>
    </header>
  );
}
