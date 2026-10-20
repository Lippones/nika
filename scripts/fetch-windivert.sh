#!/usr/bin/env bash
# Baixa o WinDivert (driver + DLL) e o instala em src-tauri/resources/windivert/.
#
# É o componente do túnel do Discord (docs/discord-tunnel.md T-07/T-11). Mesmo
# desenho do fetch-tor.sh: a versão e os SHA-256 ficam fixados em
# scripts/windivert.lock.json e qualquer byte diferente é recusado. Não há
# assinatura GPG do projeto para conferir; o que ancora a confiança é o próprio
# driver ser assinado pela Microsoft (sem isso o Windows não o carregaria) e o
# hash do zip oficial fixado por uma pessoa no --update.
#
#   scripts/fetch-windivert.sh
#   scripts/fetch-windivert.sh --version 2.2.2 --update
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/scripts/windivert.lock.json"
out="$root/src-tauri/resources/windivert"

version=""
update=0
while [ $# -gt 0 ]; do
  case "$1" in
    --version) version="$2"; shift 2 ;;
    --update) update=1; shift ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

read_lock() { python3 -c "import json;print(json.load(open('$lock')).get('$1') or '')"; }

locked_version="$(read_lock version)"
locked_hash="$(read_lock sha256)"
[ -n "$version" ] || version="$locked_version"
if [ -z "$version" ]; then
  echo "informe --version (veja https://github.com/basil00/WinDivert/releases)" >&2
  exit 2
fi

archive="WinDivert-$version-A.zip"
url="https://github.com/basil00/WinDivert/releases/download/v$version/$archive"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "→ baixando $archive"
curl -fsSL "$url" -o "$work/$archive"
hash="$(sha256sum "$work/$archive" | cut -d' ' -f1)"

if [ "$update" -eq 0 ]; then
  if [ -z "$locked_hash" ] || [ "$locked_version" != "$version" ]; then
    echo "sem checksum fixado para $version — rode com --update." >&2
    exit 1
  fi
  if [ "$hash" != "$locked_hash" ]; then
    echo "ERRO: sha256 $hash difere do fixado ($locked_hash). NÃO use este arquivo." >&2
    exit 1
  fi
  echo "→ sha256 confere com o fixado em windivert.lock.json"
fi

echo "→ extraindo x64/WinDivert.dll e x64/WinDivert64.sys"
python3 - "$work/$archive" "$work" "$version" <<'PY'
import sys, zipfile
archive, dest, version = sys.argv[1:4]
with zipfile.ZipFile(archive) as z:
    for name in (f"WinDivert-{version}-A/x64/WinDivert.dll", f"WinDivert-{version}-A/x64/WinDivert64.sys",
                 f"WinDivert-{version}-A/LICENSE"):
        data = z.read(name)
        open(f"{dest}/{name.rsplit('/', 1)[1]}", "wb").write(data)
PY

dll_hash="$(sha256sum "$work/WinDivert.dll" | cut -d' ' -f1)"
sys_hash="$(sha256sum "$work/WinDivert64.sys" | cut -d' ' -f1)"

if [ "$update" -eq 1 ]; then
  python3 - "$lock" "$version" "$hash" "$dll_hash" "$sys_hash" <<'PY'
import json, sys
path, version, digest, dll, sysf = sys.argv[1:6]
data = json.load(open(path))
data["version"], data["sha256"] = version, digest
data["files"] = {"WinDivert.dll": dll, "WinDivert64.sys": sysf}
json.dump(data, open(path, "w"), indent=2, ensure_ascii=False)
open(path, "a").write("\n")
PY
  echo "→ lock atualizado: $version / $hash"
else
  # Os hashes por arquivo são redundantes com o do zip, mas são o que o
  # revisor compara com o que está instalado na máquina.
  [ "$dll_hash" = "$(python3 -c "import json;print(json.load(open('$lock'))['files']['WinDivert.dll'])")" ] \
    || { echo "ERRO: WinDivert.dll extraída não bate com o lock" >&2; exit 1; }
  [ "$sys_hash" = "$(python3 -c "import json;print(json.load(open('$lock'))['files']['WinDivert64.sys'])")" ] \
    || { echo "ERRO: WinDivert64.sys extraído não bate com o lock" >&2; exit 1; }
fi

rm -rf "$out"
mkdir -p "$out"
cp "$work/WinDivert.dll" "$work/WinDivert64.sys" "$work/LICENSE" "$out/"
chmod -R u+rw "$out"

echo "✓ WinDivert $version instalado em $out"
