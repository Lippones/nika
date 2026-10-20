# Nika

Saída do Discord por túnel entre amigos, na bandeja do Windows.

Desde agosto de 2026 o Discord bloqueia Go Live e vídeo no Brasil. O bloqueio
está no plano de mídia: a voz e o vídeo saem do `discord_voice.node` por **UDP
direto**, com o IP real, e o servidor de mídia recusa (`receiver count: 0`).
Proxy HTTP e VPN de navegador não alcançam isso.

O Nika resolve com uma VPN **só para o Discord**: os pacotes TCP e UDP dos
processos do Discord saem por uma VM pequena e grátis na Oracle Cloud, e o resto
do PC não é tocado. Uma pessoa sobe a VM e **convida** as outras; só quem tem
convite entra.

> **Isto troca o IP do Discord. Não te torna anônimo.** O Discord passa a ver o
> IP do servidor de quem te convidou. Sua conta e seu PC continuam os mesmos.

---

## Para quem recebeu um convite

Instale o Nika, clique em **Tenho um convite**, cole a linha
`nika://convite/…` que te mandaram e aceite o UAC. Passo a passo e dúvidas em
[docs/convite.md](docs/convite.md). Nada de terminal.

## Para quem vai ter o servidor

[docs/oracle-exit.md](docs/oracle-exit.md): conta na Oracle (região home fora
do Brasil; Santiago é a mais perto), `oci session authenticate`,
`server/oracle-create.sh`, `server/install.sh "Seu nome · Região"`, e
`nika-peer add <nome>` para cada convite. Gestão de acesso é pelo terminal, de
propósito: `nika-peer remove <nome>` revoga na hora.

## Como funciona

```
Discord.exe ──pacote IP──▶ pilha do Windows ──▶ [WinDivert] ──▶ Nika
                                                               │ troca IP de origem (NAT 1:1)
                                                               │ encapsula (WireGuard, boringtun)
                                                               ▼
                                                      UDP ──▶ VM Oracle ──▶ Discord
```

- **WinDivert** (driver assinado, LGPLv3, carregado dinamicamente) entrega ao
  Nika só os pacotes dos processos `Discord*.exe`, aprendidos na camada SOCKET e
  pelas tabelas do IP Helper. Tudo o mais é devolvido ao kernel intacto.
- **NAT 1:1 em espaço de usuário**, com checksum incremental e clamp de MSS;
  destinos privados (roteador, LAN, mDNS) ficam fora do túnel.
- **WireGuard em espaço de usuário** (`boringtun`), sem adaptador nem serviço.
- **Convite** = perfil WireGuard por pessoa, também como linha única
  `nika://convite/<base64url>`, com `# Nika-Server:` e `# Nika-Peer:` para a
  tela dizer de quem é o servidor e para quem é o convite.
- **Precisa de administrador** para abrir o driver; o Nika reabre pedindo UAC.

Spec completa: [docs/discord-tunnel.md](docs/discord-tunnel.md). Decisões de
produto: [PRODUCT.md](PRODUCT.md).

> **Ainda não validado no Windows.** Perfil, NAT/checksums, convite e o
> WireGuard têm testes que rodam no Linux (inclusive um handshake real entre dois
> túneis), e o crate inteiro é type-checado no alvo Windows. O que toca o driver
> só se prova no Windows — gates em docs/discord-tunnel.md §13.

---

## Gerar o `.exe`

### 1. Pré-requisitos, uma vez só

- Node 20+, Rust estável (`rustup`), e o que o Tauri 2 pede no Windows
  (Build Tools do Visual Studio com C++ e WebView2).
- `npm ci`
- `npm run icons` (os ícones derivados não são versionados)
- `pwsh scripts/fetch-windivert.ps1` — baixa o WinDivert 2.2.2 do GitHub
  oficial e confere o SHA-256 fixado em `scripts/windivert.lock.json`.

### 2. Compilar

```bash
npm run app:build
```

O instalador NSIS (`currentUser`) sai em `src-tauri/target/release/bundle/nsis/`.
Para instalar numa máquina, o `.exe` basta: o driver vai dentro.

## Publicar uma nova versão

```bash
npm run bump 1.2.0
git commit -am "v1.2.0" && git tag v1.2.0 && git push && git push --tags
```

O workflow `release` compila no Windows, baixa o driver pelo lock, gera o
instalador e publica no GitHub Releases com o SHA-256.

## Desenvolvimento

```bash
npm run app:dev        # app Tauri com hot reload
npm run dev            # só a interface, no navegador, com núcleo falso
```

No preview web, a URL escolhe a tela: `?onboarding`, `?onboarding&step=invite`,
`?profile`, `?profile&up`.

```bash
cd src-tauri && cargo test          # roda no Linux: perfil, pacotes, WireGuard, config
cd src-tauri && cargo check --target x86_64-pc-windows-gnu   # os módulos do driver
```

### Estrutura

```
src-tauri/src/
  tunnel/        o produto: profile, packet, wg (puros) · divert, owner, elevation, engine (Windows)
  config.rs      preferências (o convite fica em tunnel.conf, fora da config)
  tray.rs        ícone e menu da bandeja
  commands.rs    IPC para o webview · actions.rs: o que UI e bandeja compartilham
src/
  components/    Stub (talão), Tunnel (faixa do convite), Onboarding, InviteInput…
  lib/           ipc.ts, types.ts, mock.ts (preview web)
server/
  oracle-create.sh · oracle-retry.sh · install.sh · nika-peer
docs/
  discord-tunnel.md · oracle-exit.md · convite.md
```

### Onde ficam os dados

`%APPDATA%\dev.nika.tortray\`: `config.json` (preferências) e `tunnel.conf`
(o convite, com a chave privada). O identificador vem da primeira versão do app
e fica assim para não mudar a pasta nem a identidade do instalador.

## Histórico

O Nika nasceu como proxy Tor na bandeja (v1.0 e v1.1), com um `version.dll`
próprio para apontar o Discord ao Tor. Isso destravava só a interface do
Discord, nunca a mídia, e foi removido em outubro de 2026. O código está no
histórico do git e o PRD original em [PRD.md](PRD.md).
