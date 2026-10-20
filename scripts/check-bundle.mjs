/**
 * Preflight do build: o Tauri falha com "resource path ... doesn't exist" se o
 * WinDivert não tiver sido baixado. Melhor avisar antes, com a receita.
 */
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const required = ["resources/windivert/WinDivert.dll", "resources/windivert/WinDivert64.sys"];

const missing = required.filter((path) => !existsSync(join(root, "src-tauri", path)));

if (missing.length > 0) {
  console.error(
    [
      "",
      "  O WinDivert (driver da saída do Discord) não está instalado. Faltam:",
      ...missing.map((path) => `    src-tauri/${path}`),
      "",
      "  Rode:  pwsh scripts/fetch-windivert.ps1   (Windows)",
      "         scripts/fetch-windivert.sh         (Linux/WSL)",
      "",
      "  Ver docs/discord-tunnel.md T-11.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}

if (process.platform !== "win32") {
  console.warn(
    "\n  AVISO: build fora do Windows. O alvo do projeto é Windows e o driver\n" +
      "  empacotado é o WinDivert64.sys — este build não gera um .exe utilizável.\n",
  );
}
