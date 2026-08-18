import type { ReactNode } from "react";

interface BandProps {
  label: string;
  action?: ReactNode;
  children: ReactNode;
}

/** Campo do bilhete: rótulo em caixa alta, ação opcional e o conteúdo. */
export function Band({ label, action, children }: BandProps) {
  return (
    <section className="band">
      <header className="band__head">
        <h2 className="label">{label}</h2>
        {action}
      </header>
      {children}
    </section>
  );
}
