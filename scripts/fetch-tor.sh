#!/usr/bin/env bash
# Baixa o Tor Expert Bundle e o instala em src-tauri/resources/.
#
# Dois modos, com garantias diferentes:
#
#   reproduzir  (padrão)  a versão e o SHA-256 já estão fixados no lock; baixa e
#                         recusa qualquer byte diferente. É o que CI faz.
#   atualizar   (--update) muda o que está fixado; aqui a assinatura GPG do
#                         projeto Tor é obrigatória, porque é o único momento em
#                         que uma âncora de confiança nova entra no repositório.
#
#   scripts/fetch-tor.sh
#   scripts/fetch-tor.sh --version 15.0.19 --update
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/scripts/tor-bundle.lock.json"
resources="$root/src-tauri/resources"

# Chave "Tor Browser Developers", que assina os pacotes oficiais.
signing_key="EF6E286DDA85EA2A4BA7DE684E2C6E8793298290"

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
  echo "informe --version (veja https://www.torproject.org/download/tor/)" >&2
  exit 2
fi

pinned=0
if [ -n "$locked_hash" ] && [ "$locked_version" = "$version" ] && [ "$update" -eq 0 ]; then
  pinned=1
fi

archive="tor-expert-bundle-windows-x86_64-$version.tar.gz"
base="https://archive.torproject.org/tor-package-archive/torbrowser/$version"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "→ baixando $archive"
curl -fsSL "$base/$archive" -o "$work/$archive"

if [ "$pinned" -eq 1 ]; then
  # O checksum fixado já foi conferido contra a assinatura por uma pessoa, no
  # --update. Bytes idênticos = mesma garantia, sem depender de keyserver.
  hash="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
  if [ "$hash" != "$locked_hash" ]; then
    echo "ERRO: sha256 $hash difere do fixado ($locked_hash). NÃO use este arquivo." >&2
    exit 1
  fi
  echo "→ sha256 confere com o fixado em tor-bundle.lock.json"
else
  if [ "$update" -eq 0 ]; then
    echo "sem checksum fixado para $version — rode com --update." >&2
    exit 1
  fi
  command -v gpg >/dev/null 2>&1 || {
    echo "gpg é obrigatório para fixar uma versão nova. Instale o gnupg." >&2
    exit 1
  }

  echo "→ verificando assinatura GPG"
  curl -fsSL "$base/$archive.asc" -o "$work/$archive.asc"
  gpg --keyserver keys.openpgp.org --recv-keys "$signing_key" >/dev/null 2>&1 || true
  gpg --verify "$work/$archive.asc" "$work/$archive"

  hash="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
  python3 - "$lock" "$version" "$hash" <<'PY'
import json, sys
path, version, digest = sys.argv[1:4]
data = json.load(open(path))
data["version"], data["sha256"] = version, digest
json.dump(data, open(path, "w"), indent=2, ensure_ascii=False)
open(path, "a").write("\n")
PY
  echo "→ lock atualizado: $version / $hash"
fi

echo "→ extraindo"
mkdir -p "$work/bundle"
tar -xzf "$work/$archive" -C "$work/bundle"

# Só o que a Fase 1 usa: o tor e as DLLs que ele carregue (versões recentes do
# bundle são estáticas e não trazem nenhuma). Ficam de fora o tor-gencert.exe
# (ferramenta de autoridade de diretório) e pluggable_transports/ (~30 MB, entra
# junto com o suporte a bridges, RF-25).
rm -rf "$resources/tor"
mkdir -p "$resources/tor"
cp "$work/bundle/tor/tor.exe" "$resources/tor/"
find "$work/bundle/tor" -maxdepth 1 -name '*.dll' -exec cp {} "$resources/tor/" \;
cp "$work/bundle/data/geoip" "$work/bundle/data/geoip6" "$resources/"
chmod -R u+rw "$resources"

echo "✓ Tor $version instalado em $resources"
