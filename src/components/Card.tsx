import type { ReactNode } from "react";

interface CardProps {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}

/** Contêiner padrão das seções da janela. */
export function Card({ title, action, children }: CardProps) {
  return (
    <section className="card">
      <header className="card__header">
        <h2>{title}</h2>
        {action}
      </header>
      {children}
    </section>
  );
}
