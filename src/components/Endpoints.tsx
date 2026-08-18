import { useState } from "react";

import { Band } from "./Band";
import { api } from "../lib/ipc";
import { httpUrl, socksUrl } from "../lib/format";
import type { Config } from "../lib/types";

interface EndpointsProps {
  config: Config;
}

/** RF-08: endereços prontos para colar em qualquer cliente. */
export function Endpoints({ config }: EndpointsProps) {
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
    <Band label="Endereços do proxy">
      <ul className="rows">
        {endpoints.map(({ label, value }) => (
          <li key={label} className="row">
            <span className="row__key">{label}</span>
            <code className="row__value row__value--strong">{value}</code>
            <button type="button" className="copy" onClick={() => void copy(value)}>
              {copied === value ? "Copiado" : "Copiar"}
            </button>
          </li>
        ))}
      </ul>
    </Band>
  );
}
