/**
 * Preflight do build: o Tauri falha com "resource path ... doesn't exist" se o
 * Tor Expert Bundle não tiver sido baixado. Melhor avisar antes, com a receita.
 */
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const required = ["resources/geoip", "resources/geoip6", "resources/tor/tor.exe"];

const missing = required.filter((path) => !existsSync(join(root, "src-tauri", path)));

if (missing.length > 0) {
  console.error(
    [
      "",
      "  O Tor Expert Bundle não está instalado. Faltam:",
      ...missing.map((path) => `    src-tauri/${path}`),
      "",
      "  Rode:  pwsh scripts/fetch-tor.ps1        (Windows)",
      "         scripts/fetch-tor.sh              (Linux/WSL)",
      "",
      "  Ver docs/tor-bundle.md.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}

// O shim do Discord é opcional: um build Tor-only funciona sem ele. Se faltar,
// avisa (não falha) — a faixa "Proxy no Discord" aparece como "componente ausente".
if (!existsSync(join(root, "src-tauri", "resources/discord/version.dll"))) {
  console.warn(
    "\n  AVISO: o shim do Discord (resources/discord/version.dll) não está\n" +
      "  presente. O proxy do Discord ficará indisponível neste build.\n" +
      "  Para incluí-lo:  pwsh scripts/build-shim.ps1\n",
  );
}

if (process.platform !== "win32") {
  console.warn(
    "\n  AVISO: build fora do Windows. O alvo do projeto é Windows e o binário do\n" +
      "  Tor empacotado é o tor.exe — este build não gera um .exe utilizável.\n",
  );
}
