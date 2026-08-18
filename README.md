# Nika

Proxy Tor na bandeja do Windows. Sobe junto com o sistema, conecta em segundo
plano e deixa um **SOCKS5 em `127.0.0.1:9050`** e um **HTTP CONNECT em
`127.0.0.1:9080`** prontos para qualquer aplicativo apontar — sem abrir o Tor
Browser, sem driver, sem privilégio de administrador.

Implementa a Fase 1 do [PRD](./PRD.md), mais o **proxy no Discord** (Fase 4 —
ver abaixo).

> **Isto troca o seu IP, não te torna anônimo.** Para navegação anônima, use o
> Tor Browser.

---

## Proxy no Discord

O Discord não tem configuração de proxy e ignora o proxy de sistema. O Nika
resolve isso instalando, dentro da pasta do Discord, uma **`version.dll` própria**
(o *shim*, crate [`discord-shim`](src-tauri/discord-shim/)): ao carregar, ela
injeta `--proxy-server` no Chromium do Electron. Diferente da abordagem antiga
(baixar o binário do [discord-drover](https://github.com/hdrover/discord-drover)),
o shim é nosso — entra no instalador, pode ser assinado e falha aberto (sem
proxy) em vez de derrubar o Discord.

Na janela do Nika, a faixa **Proxy no Discord** detecta as pastas, o processo
aberto e a porta — instalar e remover são um clique.

> **Voz não passa pelo Tor.** A voz do Discord é UDP e o Tor só transporta TCP:
> entrar em um canal de voz revela seu IP real. E o Discord trata IP de saída
> Tor com hostilidade (captcha, verificação por telefone, bloqueio de conta).

> **Ainda não validado dentro de um Discord real.** A DLL compila e é
> type-checada no alvo Windows, mas hookar sem quebrar o processo só se prova no
> Windows — ver os gates em [docs/discord-dll.md](docs/discord-dll.md) §12.

Spec da DLL: [docs/discord-dll.md](docs/discord-dll.md). Descoberta de pastas,
processo e instalar/remover: [docs/discord-proxy.md](docs/discord-proxy.md).

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
pwsh scripts/build-shim.ps1    # compila o version.dll do proxy do Discord (opcional)
```

A versão e o checksum do Tor já estão fixados em `scripts/tor-bundle.lock.json`;
o script baixa exatamente aquilo e recusa qualquer coisa diferente. Para subir
de versão, ver [docs/tor-bundle.md](docs/tor-bundle.md).

`build-shim.ps1` é **opcional**: sem ele o app compila e roda, só a faixa
"Proxy no Discord" aparece como "componente ausente". Com ele, o `version.dll`
do shim vai para `resources/discord/` e entra no instalador.

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

### Ver só a interface, no navegador

```bash
npm run dev
```

Fora do Tauri não existe core, então `src/lib/ipc.ts` cai num núcleo falso
(`src/lib/mock.ts`): ele simula o bootstrap, entrega um circuito e um log
plausíveis e faz o tráfego subir. Serve para mexer no design sem compilar o app
— e não vai para dentro do Tauri, onde o `invoke` de verdade é usado.

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
src-tauri/icons/
  source/            as duas nuvens de meio-tom (arte de origem, versionada)
  *.png, icon.ico    derivados por `npm run icons` — não edite à mão
src/
  lib/ipc.ts         única porta de entrada para o core
  lib/mock.ts        núcleo falso do preview web (nunca usado dentro do Tauri)
  hooks/             estado do Tor, logs, config, ações
  components/        campos do bilhete (talão, endereços, circuito, selo)
  styles.css         o sistema visual inteiro, em tokens
  fonts/             JetBrains Mono e Inter Tight (offline, ver fonts/README.md)
  assets/backdrop.jpg  cena de fundo do talão — troque o arquivo para mudar
```

### Ícones

A arte é uma nuvem em meio-tom e mora em `src-tauri/icons/source/`: `app-icon.png`
(nuvem clara sobre o cartão escuro) e `tray-cloud.png` (a nuvem solta).
`npm run icons` decodifica, recorta pela caixa da tinta, reamostra por média de
caixa e escreve todos os tamanhos, inclusive o `.ico`. Trocar um ícone é trocar o
PNG de origem e rodar isso — nada de editar os derivados.

Na bandeja a retícula não sobrevive: a 16px ela vira borrão. Então de lá sai só a
**silhueta chapada** da mesma nuvem, e o estado é dito pela forma, nunca por cor:

| Estado | Desenho |
|---|---|
| `stopped` | só o contorno, em `#9A9A9A` |
| `connecting` | cheia em `#C4C4C4`, aro escuro |
| `connected` | cheia em `#F5F5F5`, aro `#565656` |
| `error` | cheia, cortada por um vão diagonal |

O aro escuro existe para a barra de tarefas clara, onde branco sobre branco
desapareceria. O corte do erro é um vão, não uma barra colorida, pelo mesmo
motivo.

**Se o executável continuar com o ícone antigo**, não é o `.ico`: é cache. O
`tauri_build` embute `icons/icon.ico` no binário mas não pedia rebuild quando a
arte mudava — [`build.rs`](src-tauri/build.rs) agora declara essa dependência. Já
uma instalação anterior segue com o ícone dela até ser reinstalada, e o Explorer
guarda miniatura à parte (`ie4uinit.exe -show` limpa).

### Desenho da janela

A janela é um **bilhete técnico**: barra de cabeçalho com o estado, talão com o
estado em letra grande e um globo em ASCII, picote, campos rotulados em caixa
alta e um selo com o aviso obrigatório do PRD (§4). Paleta neutra, sem cor de
acento: a hierarquia é feita de fios de 1px e de uma única inversão
(`#F5F5F5` sobre `#191919`), reservada à ação principal.

Dois pontos que valem saber antes de mexer:

- **O código de barras é dado.** Cada barra vem de um dígito hexadecimal do
  fingerprint do relay de saída (`src/components/Barcode.tsx`) — circuito novo,
  código novo.
- **Sem verde nem vermelho.** Como não existe cor de acento, "fora do Tor" e os
  erros gritam por inversão de contraste, não por cor.

### Primeira execução (onboarding)

Quando `config.onboarded` é `false` — instalação nova, ou config da v1 onde o
campo não existe e o `serde(default)` o preenche — a UI abre no
`Onboarding` (`src/components/Onboarding.tsx`) em vez da janela. São duas telas
do mesmo bilhete: boas-vindas (explicação curta + o selo do aviso) e o Discord
(os avisos honestos da spec e o install). O Tor conecta dobrado no gesto de ir
para o Discord, e o botão de instalar espera o proxy subir. Concluir ou pular
grava `onboarded: true` via `set_config`; a partir daí a janela normal abre
direto. A referência visual das telas está em
[docs/onboarding-v2.html](docs/onboarding-v2.html) (abre em
`http://localhost:5173/docs/onboarding-v2.html` com `npm run dev`; a própria UI
em `?onboarding` força a primeira execução no preview).

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
