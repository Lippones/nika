import { useEffect, useRef, useState } from "react";

import { Card } from "./Card";
import { useLogs } from "../hooks/useLogs";

/** RF-17: as últimas linhas do tor, para diagnóstico. */
export function LogCard() {
  const lines = useLogs();
  const [open, setOpen] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) endRef.current?.scrollIntoView({ block: "end" });
  }, [lines, open]);

  return (
    <Card
      title="Log do Tor"
      action={
        <button type="button" className="ghost" onClick={() => setOpen(!open)}>
          {open ? "Ocultar" : `Mostrar (${lines.length})`}
        </button>
      }
    >
      {open && (
        <div className="log">
          {lines.map((line, index) => (
            <p key={`${index}-${line}`}>{line}</p>
          ))}
          <div ref={endRef} />
        </div>
      )}
    </Card>
  );
}
