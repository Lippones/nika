# Tor Expert Bundle

O binário do Tor **não** é versionado. Ele é baixado do projeto Tor, verificado
e instalado em `src-tauri/resources/` por `scripts/fetch-tor.ps1` (Windows) ou
`scripts/fetch-tor.sh` (Linux/WSL).

## Layout instalado

```
src-tauri/resources/
  geoip        bases de GeoIP, usadas para mostrar o país de cada nó
  geoip6
  tor/
    tor.exe    e as DLLs que ele carregue
```

Não vai tudo que o bundle traz. Ficam de fora:

| Item | Por quê |
|---|---|
| `tor-gencert.exe` (~6 MB) | ferramenta de autoridade de diretório; um cliente nunca usa |
| `pluggable_transports/` (~30 MB) | só serve para bridges (RF-25), que é Fase 2 |

Com isso o payload fica em ~35 MB antes da compressão do NSIS. Ao implementar
bridges, é aqui e no glob de `resources` do `tauri.conf.json` que o
`lyrebird.exe` volta.

O `tor.exe` vai numa pasta própria, e não como `externalBin`, porque o Expert
Bundle não é um binário solto: ele depende de DLLs que precisam estar no mesmo
diretório, e `externalBin` instala um arquivo só. `paths::tor_binary` procura
nos dois layouts.

## Atualizar a versão

1. Veja a versão atual em https://www.torproject.org/download/tor/.
2. Rode com `--update`:

   ```powershell
   pwsh scripts/fetch-tor.ps1 -Version <nova> -Update
   ```

   ```bash
   scripts/fetch-tor.sh --version <nova> --update
   ```

3. O script baixa o `.tar.gz` e o `.asc`, **verifica a assinatura GPG** contra a
   chave `EF6E286DDA85EA2A4BA7DE684E2C6E8793298290` (Tor Browser Developers),
   calcula o SHA-256 e grava versão + checksum em `scripts/tor-bundle.lock.json`.
4. Commite o lock. A partir daí, qualquer execução sem `--update` **falha** se o
   checksum não bater — é isso que protege contra um binário adulterado.

Sem `gpg` instalado o script avisa e segue apenas com o checksum. Para gerar
release, instale o Gpg4win (Windows) ou o pacote `gnupg` (Linux) e confira que a
verificação passou antes de fixar o lock.

## Por que isso importa

Um `tor.exe` adulterado enxerga todo o tráfego que passa pelo proxy. É o risco
crítico da tabela do PRD (§10), e o único controle que temos é: baixar da fonte
oficial, verificar a assinatura e fixar o checksum.
