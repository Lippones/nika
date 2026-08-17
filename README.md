# Nika

Proxy Tor na bandeja do Windows. Sobe junto com o sistema, conecta em segundo
plano e deixa um **SOCKS5 em `127.0.0.1:9050`** e um **HTTP CONNECT em
`127.0.0.1:9080`** prontos para qualquer aplicativo apontar — sem abrir o Tor
Browser, sem driver, sem privilégio de administrador.

Implementa a Fase 1 do [PRD](./PRD.md).

> **Isto troca o seu IP, não te torna anônimo.** Para navegação anônima, use o
> Tor Browser.

---

## Gerar o `.exe`

O build precisa rodar **no Windows** (o instalador NSIS e o `tor.exe` são
Windows-only; a árvore pode viver no WSL, mas compile do lado Windows).

### 1. Pré-requisitos, uma vez só

| O quê | Como |
|---|---|
| Rust (MSVC) | https://rustup.rs → `rustup default stable-x86_64-pc-windows-msvc` |
| Build Tools do VS | "Desktop development with C++" (traz o linker) |
| Node 20+ | https://nodejs.org |
| WebView2 | já vem no Windows 11; no 10, instalar o Evergreen Runtime |

### 2. Preparar a árvore

```powershell
npm install
npm run icons                  # gera os ícones do app e da bandeja
pwsh scripts/fetch-tor.ps1     # baixa e verifica o Tor Expert Bundle (~35 MB)
```

A versão e o checksum já estão fixados em `scripts/tor-bundle.lock.json`; o
script baixa exatamente aquilo e recusa qualquer coisa diferente. Para subir de
versão, ver [docs/tor-bundle.md](docs/tor-bundle.md).

### 3. Compilar

```powershell
npm run app:build
```

Saída:

```
src-tauri/target/release/nika.exe                              executável solto
src-tauri/target/release/bundle/nsis/Nika_0.1.0_x64-setup.exe  instalador
```

O instalador é `currentUser`: instala em `%LOCALAPPDATA%\Nika` sem UAC.

## Publicar uma nova versão

O build oficial nasce da CI, não da sua máquina:

```bash
npm run bump 0.2.0                  # package.json, Cargo.toml, Cargo.lock, tauri.conf.json
git commit -am "v0.2.0"
git tag v0.2.0
git push --follow-tags
```

A tag dispara [`release.yml`](.github/workflows/release.yml): compila no
`windows-latest`, baixa o Tor exatamente como fixado no lock e publica no GitHub
Releases o instalador e o `.sha256` dele.

Para exercitar o pipeline sem criar versão: aba **Actions → release → Run
workflow**. Compila e sobe o instalador como artefato, sem publicar release.

Todo push e PR passam por [`ci.yml`](.github/workflows/ci.yml): typecheck do
frontend, `cargo fmt --check`, `clippy -D warnings` e `cargo test` no Windows.

Duas coisas que valem separar:

- **Atualizar o Tor embutido** é outro eixo, descrito em
  [docs/tor-bundle.md](docs/tor-bundle.md). Depois de mexer no lock, faça também
  um bump do app — o instalador mudou.
- **O app não se auto-atualiza.** Está fora da Fase 1; quem instalou baixa o
  `.exe` novo. Se virar requisito, o caminho é o `tauri-plugin-updater` lendo um
  JSON do próprio Releases, e aí passa a exigir chave de assinatura.

## Desenvolvimento

### Rodar localmente

```powershell
npm run app:dev
```

### Testes

```bash
npm run build          # typecheck + bundle do frontend
cargo test --manifest-path src-tauri/Cargo.toml   # parsing do protocolo, torrc, backoff
```

Fora do Windows dá para conferir o código do lado Rust sem compilar o app
inteiro, contanto que exista um compilador C para o host:

```bash
rustup target add x86_64-pc-windows-gnu
cargo clippy --manifest-path src-tauri/Cargo.toml \
  --target x86_64-pc-windows-gnu --all-targets -- -D warnings
```

Isso checa tipos de tudo, inclusive `platform/windows.rs` e os testes. Rodar os
testes de fato exige Windows.

---

## Como funciona

```
┌── Tauri (Rust) ─────────────────────────────┐
│  supervisor ──spawn/kill──► tor.exe         │
│      │                        │ 9050 SOCKS5 │
│      │                        │ 9080 HTTP   │
│      └──control────► ControlPort 9051       │
│                                             │
│  state ──eventos──► WebView (React)         │
│        └──────────► ícone da bandeja        │
└─────────────────────────────────────────────┘
```

O **supervisor** (`src-tauri/src/supervisor.rs`) é um ator: uma única tarefa é
dona do processo do tor, do contador de tentativas e dos prazos. Todo mundo
fala com ele por mensagem, então não existe estado mutável compartilhado nem
como duas partes do app subirem dois `tor.exe`.

O **estado** (`state.rs`) tem uma escrita só, `StatusStore::update`, que
publica para a UI e para a bandeja no mesmo passo — os dois nunca divergem.

### Estrutura

```
src-tauri/src/
  supervisor.rs      ciclo de vida do tor: spawn, vigia, backoff, kill
  control/           protocolo do ControlPort (parser, cliente, eventos, GETINFO)
  state.rs           estado observável + o que é compartilhado
  config.rs          configuração do usuário em JSON
  torrc.rs           geração do torrc a cada start
  ports.rs           checagem de porta ocupada
  platform/          Job Object do Windows (mata o tor junto com o app)
  actions.rs         ações compartilhadas entre UI e bandeja
  commands.rs        superfície de IPC
  tray.rs            ícone e menu da bandeja
src/
  lib/ipc.ts         única porta de entrada para o core
  hooks/             estado do Tor, logs, config, ações
  components/        cartões da janela
```

### Requisitos cobertos

| Fase 1 (MVP) | Onde |
|---|---|
| RF-01 a RF-03 — spawn, kill sem órfão, restart com backoff | `supervisor.rs`, `platform/windows.rs` |
| RF-04 — instância única | `lib.rs` (`tauri-plugin-single-instance`) |
| RF-05 — porta ocupada com mensagem clara | `ports.rs` |
| RF-06, RF-07, RF-10 — SOCKS5, HTTP tunnel, bind só em loopback | `torrc.rs` |
| RF-11, RF-12 — bootstrap por evento, auth por cookie | `control/` |
| RF-13 — nova identidade | `actions.rs` |
| RF-18 a RF-23 — bandeja, autostart, esconder no X | `tray.rs`, `autostart.rs`, `lib.rs` |
| RF-24 — configuração persistida | `config.rs` |

Também entraram, por saírem de graça na mesma estrutura: RF-08 (copiar
endereço), RF-14 (circuito com países), RF-15 (IP de saída sob demanda),
RF-16 (tráfego) e RF-17 (painel de log).

Fora do escopo desta fase: bridges obfs4 (RF-25), `DNSPort` (RF-09) e escolha de
país de saída (RF-26).

### Onde ficam os dados

```
%APPDATA%\dev.nika.tortray\
  config.json    portas, autostart, autoconnect
  torrc          regerado a cada start — editar não adianta
  tor\           DataDirectory do tor (inclui o cookie do ControlPort)
```

---

## Verificação rápida

```powershell
curl --socks5-hostname 127.0.0.1:9050 https://check.torproject.org/api/ip
# {"IsTor":true,"IP":"..."}
```

Matar o app pelo Gerenciador de Tarefas não deve deixar `tor.exe` rodando — é o
que o Job Object garante.
