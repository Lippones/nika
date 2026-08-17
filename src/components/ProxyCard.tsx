import { useState } from "react";

import { Card } from "./Card";
import { api } from "../lib/ipc";
import { httpUrl, socksUrl } from "../lib/format";
import type { Config } from "../lib/types";

interface ProxyCardProps {
  config: Config;
}

/** RF-08: endereços prontos para colar em qualquer cliente. */
export function ProxyCard({ config }: ProxyCardProps) {
  const [copied, setCopied] = useState<string | null>(null);

  async function copy(value: string) {
    await api.copyText(value);
    setCopied(value);
    window.setTimeout(() => setCopied((current) => (current === value ? null : current)), 1500);
  }

  const endpoints = [
    { label: "SOCKS5", value: socksUrl(config) },
    { label: "HTTP", value: httpUrl(config) },
  ];

  return (
    <Card title="Endereços do proxy">
      <ul className="endpoints">
        {endpoints.map(({ label, value }) => (
          <li key={label}>
            <span className="endpoints__label">{label}</span>
            <code>{value}</code>
            <button type="button" onClick={() => void copy(value)}>
              {copied === value ? "Copiado" : "Copiar"}
            </button>
          </li>
        ))}
      </ul>
    </Card>
  );
}
