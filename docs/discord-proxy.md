# Proxy do Discord — spec de implementação

**Status:** Implementado (RF-27 a RF-39); ver §15
**Data:** 2026-08-17
**Depende de:** PRD §6.2 (proxy), §7 (offline-first), §10 (riscos)
**Numeração:** RF-27 a RF-45, continuando o PRD.
**Atualização:** a decisão D-02 (baixar o binário do drover) foi substituída por
uma `version.dll` própria — ver [docs/discord-dll.md](discord-dll.md). O resto
desta spec (descoberta, processo, instalar/remover) segue valendo.

---

## 1. Objetivo

Fazer o tráfego TCP do app do Discord passar pelo proxy que o Nika já expõe,
com **instalação e remoção em um clique** a partir da própria janela do Nika:
detectar as pastas do Discord, detectar se ele está aberto, fechar, aplicar,
reabrir — sem o usuário digitar host, porta ou caminho.

O Discord não tem configuração de proxy e ignora o proxy de sistema do Windows.
A única via prática é a que o [discord-drover](https://github.com/hdrover/discord-drover)
usa: uma DLL que o processo do Discord carrega e que injeta o proxy de dentro.

## 2. Mecanismo (o que a DLL faz)

`version.dll` é colocada ao lado de `Discord.exe`. O `Discord.exe` importa
`version.dll`; como ela **não** é uma KnownDLL, o Windows resolve primeiro no
diretório do executável e carrega a nossa. Ela recarrega a original de
`%SystemRoot%\System32\version.dll`, reexporta as 17 funções e instala hooks
inline:

| Hook | Efeito |
|---|---|
| `GetCommandLineW` | acrescenta `--proxy-server=<proto>://<host>:<porta>` — é isto que faz o Chromium do Electron proxyar tudo |
| `GetEnvironmentVariableW` | responde `http_proxy`/`https_proxy` para o lado Node/updater |
| `CreateProcessW` | recopia DLL + ini para pastas `app-*` novas → sobrevive ao update do Discord |
| `send`/`recv` | converte o handshake `CONNECT host:porta` em SOCKS5 quando o proxy configurado é SOCKS5 |
| `socket`/`WSASocket`/`WSASend`/`WSASendTo` | contabilidade de sockets e o truque de UDP para desbloquear voz |

Configuração: um `drover.ini` de três linhas ao lado do `Discord.exe`.

```ini
[drover]
; Proxy can use http or socks5 protocols
proxy = http://127.0.0.1:9080
```

## 3. Decisões de projeto

| # | Decisão | Por quê |
|---|---|---|
| D-01 | **Não** reimplementar a DLL. Reusar o `version.dll` compilado do drover | Reimplementar = engine de hook inline x64, 17 exports, depuração dentro do processo do Discord: 2–4 semanas contra 3–5 dias, sem ganho para o usuário |
| D-02 | O componente **não vai no instalador**. É baixado sob demanda, pelo próprio Tor, com SHA-256 fixado | O repo do drover não tem `LICENSE` (all rights reserved): redistribuir o binário não é permitido. E o PRD §7 exige que o app não faça chamada de rede fora do Tor — baixar *pelo* Tor respeita as duas coisas |
| D-03 | Proxy padrão = **`http://127.0.0.1:{httpPort}`** (o `HTTPTunnelPort` do Tor), não o SOCKS | No modo SOCKS5 o drover reescreve o `CONNECT` em pacote SOCKS5 dentro do hook de `send` — o próprio código admite risco de misturar a resposta do proxy com dados do servidor. Com HTTP tunnel, Chromium e Node falam CONNECT nativo e o DNS continua resolvido pelo Tor |
| D-04 | Instalar em **todas** as pastas `app-*` de todos os sabores encontrados | É o que o drover faz; evita o caso de o Discord abrir de uma pasta antiga |
| D-05 | Fechar o Discord = **matar o processo**, não `WM_CLOSE` | O Discord minimiza para a bandeja no `WM_CLOSE`; esperar fechamento gracioso trava a instalação. Electron reabre limpo depois de morto |
| D-06 | Reabrir via `Update.exe --processStart <exe>` | É como o atalho do menu Iniciar abre; abrir o `Discord.exe` direto pula o updater |
| D-07 | Estado do Discord é **derivado do disco**, não persistido | A verdade é: existe `version.dll` na pasta e o que diz o `drover.ini`. Persistir "instalado: sim" mente quando o Discord se atualiza ou o usuário reinstala |
| D-08 | O modo `direct` do drover (sem proxy, só o truque de UDP) fica fora | Nada a ver com o propósito do Nika |

## 4. O que o usuário precisa saber (vai na UI, não só aqui)

1. **Voz não passa pelo Tor.** O Tor só transporta TCP; a voz do Discord é UDP e
   sai direto pela conexão do usuário, com o IP real. Com o proxy ativo,
   entrar em canal de voz **anula o efeito do proxy** para quem observa.
2. **O Discord trata IP de saída Tor com hostilidade**: captcha em loop,
   verificação por telefone e bloqueio de conta são resultados comuns.
3. Instalar isto **modifica a pasta de instalação do Discord** com uma técnica
   (DLL ao lado do executável) que antivírus classificam como suspeita.

## 5. Escopo

### 5.1 Requisitos

| ID | Requisito | Prioridade |
|---|---|---|
| RF-27 | Detectar instalações do Discord (Stable, Canary, PTB) e suas pastas `app-*` sem o usuário informar caminho | Must |
| RF-28 | Detectar se o Discord está em execução, antes de instalar ou remover | Must |
| RF-29 | Fechar o Discord a pedido do usuário (confirmação explícita) e reabri-lo depois da operação | Must |
| RF-30 | Instalar: gravar `drover.ini` + copiar `version.dll` e `drover-packet.bin` em todas as pastas `app-*` detectadas | Must |
| RF-31 | Remover: apagar os três arquivos de todas as pastas `app-*` | Must |
| RF-32 | Preencher o endereço do proxy a partir da config do Nika — sem campo de host/porta na UI | Must |
| RF-33 | Baixar o componente (`version.dll`, `drover-packet.bin`) sob demanda, **através do SOCKS do Tor**, verificando SHA-256 fixado no binário | Must |
| RF-34 | Recusar qualquer arquivo cujo hash não bata e não deixar resíduo em disco | Must |
| RF-35 | Exibir estado real lido do disco: instalado onde, com qual proxy, componente presente/íntegro, Discord aberto | Must |
| RF-36 | Bloquear a instalação enquanto o Tor não estiver conectado, com mensagem clara | Must |
| RF-37 | Avisar, de forma permanente enquanto o proxy estiver instalado, que voz UDP sai fora do Tor | Must |
| RF-38 | Reescrever o `drover.ini` quando as portas do Tor mudarem, sinalizando que o Discord precisa reiniciar | Should |
| RF-39 | Revalidar na abertura do app e reaplicar em pastas `app-*` novas (Discord atualizado), se o Discord estiver fechado | Should |
| RF-40 | Ação equivalente no menu da bandeja (ligar/desligar proxy do Discord) | Could |
| RF-41 | Importar o componente de um `.zip` local, para quem não quer baixar pelo app | Could |

### 5.2 Fora de escopo

| Item | Motivo |
|---|---|
| Reimplementar a DLL | D-01 |
| Fazer voz funcionar sobre Tor | Impossível: Tor é TCP-only |
| Modo `direct` (só bypass de UDP) | D-08 |
| Discord instalado por winget/Store/portátil fora de `app-*` | Layout diferente; detectar e avisar, não suportar |
| Vencer o bloqueio de Tor do Discord (captcha, verificação) | Fora do nosso controle |

## 6. Arquitetura

```
src-tauri/src/discord/
  mod.rs         tipos públicos, DiscordState (cache do último scan) e orquestração
  discover.rs    registro + varredura de app-*; escolha de versão
  process.rs     detectar / matar / reabrir o Discord (Toolhelp32)
  install.rs     escrever ini, copiar, apagar, conferir o que está instalado
  component.rs   baixar o zip pelo Tor, verificar SHA-256, extrair, manifesto
```

Segue o mesmo desenho do resto do core: `commands.rs` é casca, `actions.rs`
concentra o que UI e bandeja compartilham, o estado observável é emitido para o
webview por evento.

```
UI  ──invoke──▶ commands::discord_*  ──▶ discord::{discover,install,process,component}
                                              │
                                              ▼
                                    %APPDATA%\dev.nika.tortray\drover\
                                              │ copia
                                              ▼
                              %LOCALAPPDATA%\Discord\app-1.0.9xxx\
                                 version.dll · drover.ini · drover-packet.bin
```

### 6.1 Layout em disco

```
%APPDATA%\dev.nika.tortray\
  drover\
    version.dll          componente baixado e verificado
    drover-packet.bin
    manifest.json        { "version": "0.9", "files": { "<nome>": "<sha256>" } }
```

O `manifest.json` registra versão e origem, e é dele que a UI tira o "drover
0.9". Ele **não** é usado como prova de integridade: quem decide se o
componente está íntegro é sempre o SHA-256 dos arquivos, recalculado a cada
varredura. São ~5 ms para 2,3 MB — barato demais para justificar um cache que
poderia mentir.

## 7. Contratos

### 7.1 Tipos (Rust, `discord/mod.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Flavor { Stable, Canary, Ptb }

/// Modo pedido pelo usuário; mora na config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Off,
    /// `http://127.0.0.1:{httpPort}` — padrão (D-03).
    TorHttp,
    /// `socks5://127.0.0.1:{socksPort}` — só para quem sabe o que está fazendo.
    TorSocks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDir {
    pub path: PathBuf,
    /// Versão extraída de `app-1.0.9186`, para ordenar.
    pub version: String,
    /// `version.dll` presente **e** com o hash do componente atual.
    pub installed: bool,
    /// Valor de `proxy =` lido do `drover.ini`, se houver.
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub flavor: Flavor,
    /// "Discord", "Discord Canary", "Discord PTB" — para a UI não repetir isso.
    pub label: &'static str,
    pub base_dir: PathBuf,
    pub exe_name: &'static str,
    pub update_exe: Option<PathBuf>,
    /// Ordenado da versão mais nova para a mais antiga.
    pub app_dirs: Vec<AppDir>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Component { Missing, Ready, Corrupt }

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscordStatus {
    pub component: Component,
    pub component_version: Option<String>,
    pub installs: Vec<Install>,
    /// Algum processo Discord/DiscordCanary/DiscordPTB vivo.
    pub running: bool,
    /// Modo efetivamente instalado, derivado do disco (D-07) — não o da config.
    pub effective: Mode,
    /// Instalado, mas apontando para porta diferente da config atual (RF-38).
    pub stale: bool,
}
```

### 7.2 Config (`config.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DiscordConfig {
    pub mode: discord::Mode,
    /// RF-39: revalidar e reaplicar na abertura do app.
    pub reapply_on_start: bool,
    /// Fechar o Discord sem perguntar de novo nas próximas operações.
    pub allow_close: bool,
}
```

Entra em `Config` como `pub discord: DiscordConfig`. `Config` já usa
`#[serde(default)]`, então `config.json` antigo continua carregando.

### 7.3 Comandos (`commands.rs`) e evento

| Comando | Assinatura | Notas |
|---|---|---|
| `discord_status` | `() -> Result<DiscordStatus>` | força um scan; barato (só `read_dir` + registro + um SHA-256 por pasta) |
| `discord_fetch_component` | `() -> Result<DiscordStatus>` | RF-33; exige `Phase::Connected` |
| `discord_install` | `(mode, close_discord, relaunch) -> Result<DiscordStatus>` | RF-30; exige `Phase::Connected` e componente `Ready` |
| `discord_uninstall` | `(close_discord, relaunch) -> Result<DiscordStatus>` | RF-31; não exige Tor no ar |
| `discord_relaunch` | `() -> Result<DiscordStatus>` | RF-29 |

Evento: `nika://discord`, payload `DiscordStatus`, emitido por
`DiscordState::update` — mesmo padrão de `StatusStore` em `state.rs`.

### 7.4 Erros novos (`error.rs`)

```rust
#[error("não encontrei o Discord instalado neste usuário")]
DiscordNotFound,

#[error("o Discord está aberto — feche-o para continuar")]
DiscordRunning,

#[error("o componente do proxy do Discord ainda não foi baixado")]
DroverMissing,

#[error("o arquivo baixado não confere (esperado {expected}, veio {actual}) — nada foi instalado")]
DroverChecksum { expected: String, actual: String },

#[error("não consegui gravar em: {paths}")]
DiscordWriteFailed { paths: String },
```

### 7.5 Frontend

`src/lib/types.ts` ganha os espelhos de `Mode`, `Component`, `AppDir`,
`Install`, `DiscordStatus`; `src/lib/ipc.ts` ganha os cinco comandos e
`onDiscord(handler)`. Novo hook `src/hooks/useDiscord.ts` (mesma forma de
`useTorStatus`: snapshot inicial + assinatura do evento) e novo componente
`src/components/DiscordProxy.tsx` usando `Band`.

## 8. Algoritmos

### 8.1 Descoberta (`discover.rs`, RF-27)

Para cada sabor — `Discord`/`Discord.exe`, `DiscordCanary`/`DiscordCanary.exe`,
`DiscordPTB`/`DiscordPTB.exe`:

1. Candidatos a diretório base, nesta ordem, deduplicados sem diferenciar caixa:
   - `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{Flavor}` →
     valor `InstallLocation`
   - `HKCU\Software\Classes\{Flavor}\shell\open\command` → valor padrão, do qual
     se extrai o prefixo antes de `app-`: `^"(.+\\)app-`
   - `%LOCALAPPDATA%\{Flavor}\` (fallback quando o registro está sujo)
2. Em cada base existente, listar subpastas `app-*` que contenham o `.exe` do sabor.
3. Versão = `app-(\d+(?:\.\d+)*)`, comparada componente a componente (numérica,
   não lexicográfica — `app-1.0.9186` > `app-1.0.972`). Ordenar decrescente.
4. `update_exe` = `base_dir\Update.exe`, se existir.
5. Nenhuma pasta em nenhum sabor → `Error::DiscordNotFound`.

Só `HKCU`: o Discord instala por usuário, em `%LOCALAPPDATA%`. É o que mantém a
promessa de "sem admin" do PRD §7.

### 8.2 Estado de cada pasta (`install.rs`, RF-35)

- `installed` = existe `version.dll` **e** o SHA-256 bate com o do componente
  (uma `version.dll` de outra origem não conta como nossa instalação).
- `proxy` = valor de `proxy =` no `drover.ini`, parseado com um leitor de INI
  mínimo (uma seção, uma chave — não vale trazer dependência para isto).
- `effective` = `TorHttp`/`TorSocks` conforme o esquema da URL da pasta mais nova
  instalada; `Off` se nenhuma.
- `stale` = `installed` e a porta do `proxy` difere da porta correspondente na
  config atual.

### 8.3 Processo (`process.rs`, RF-28/29)

- **Detectar:** `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` e comparar
  `szExeFile` (sem caixa) com os três nomes.
- **Fechar:** para cada PID, `OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE)` →
  `TerminateProcess` → `WaitForSingleObject` com 5s. Depois, esperar até 3s
  (poll de 100 ms) até o snapshot ficar limpo — o Discord tem vários processos
  filhos com o mesmo nome, e os arquivos só destravam quando todos saem.
  Nunca fazer isso sem `close_discord == true` vindo de confirmação explícita.
- **Reabrir:** `update_exe` com `--processStart <exe_name>`; sem `Update.exe`,
  o `.exe` da pasta mais nova. Sempre desanexado (não é filho do Nika, não pode
  cair junto com o Job Object do Tor).

### 8.4 Instalar (RF-30)

Pré-condições, nesta ordem: `component == Ready`; existe pelo menos uma pasta;
Tor em `Phase::Connected` (RF-36); Discord fechado ou `close_discord`.

Para cada pasta `app-*` de cada sabor:

1. Gravar `drover.ini` — arquivo pequeno, escrita direta, `\r\n`:
   ```ini
   [drover]
   ; gerado pelo Nika — não editar
   proxy = http://127.0.0.1:9080
   ```
2. Copiar `version.dll` e `drover-packet.bin` do diretório do componente,
   sobrescrevendo.
3. Acumular falhas com o caminho; ao final, se houver, `DiscordWriteFailed`
   listando o que falhou — e **não** reverter o que deu certo (uma pasta com os
   arquivos corretos nunca é pior que uma pasta pela metade; o estado real
   aparece no próximo scan).

Depois: persistir `discord.mode` na config, reemitir status, reabrir o Discord
se `relaunch`.

### 8.5 Remover (RF-31)

Mesmas pré-condições de processo. Apagar `version.dll`, `drover.ini` e
`drover-packet.bin` de **todas** as pastas de todos os sabores — inclusive
pastas antigas onde o usuário instalou por outro caminho. `NotFound` não é
erro. Ao final, `discord.mode = Off`.

### 8.6 Componente (`component.rs`, RF-33/34)

Constantes compiladas no binário:

```rust
const DROVER_VERSION: &str = "0.9";
const DROVER_URL: &str =
    "https://github.com/hdrover/discord-drover/releases/download/v0.9/drover-v0.9.zip";
const DROVER_ZIP_SHA256: &str =
    "9bd4f5e27ccd0e195ea0ffc5822d3057d8aad700b44b97067ab91fdb67c861a1";
const DLL_SHA256: &str =
    "4ec839f1ecb25e7befb3a52d6ce521b0ec946da860002efeb527def045bd31c8";
const PACKET_SHA256: &str =
    "f4589c57749f956bb30538197a521d7005f8b0a8723b4707e72405e51ddac50a";
const MAX_ZIP_BYTES: u64 = 8 * 1024 * 1024;
```

Fluxo:

1. Exigir `Phase::Connected`. O download sai pelo `socks5h://127.0.0.1:{socksPort}`,
   com o mesmo `reqwest` já usado em `check_exit_ip` — nenhuma chamada de rede
   fora do Tor (PRD §7).
2. Baixar para `%APPDATA%\dev.nika.tortray\drover\.tmp\drover.zip`, abortando
   acima de `MAX_ZIP_BYTES`; timeout de 180s (é Tor, não fibra).
3. SHA-256 do zip inteiro. Divergiu → apagar o `.tmp` e `DroverChecksum`.
4. Extrair **apenas** `drover/version.dll` e `drover/drover-packet.bin`, por
   nome exato: nada de iterar entradas e escrever onde o zip mandar (path
   traversal). Descartar `drover.exe` — é o instalador VCL, não usamos.
5. SHA-256 de cada arquivo extraído contra as constantes.
6. Mover para o diretório final, escrever `manifest.json`, apagar `.tmp`.
7. Qualquer falha: `.tmp` some, o diretório final não é tocado.

O par "hash do zip + hash de cada arquivo" é redundante de propósito: o hash do
zip detecta troca de release, o dos arquivos protege da extração.

**Atualizar a versão do drover:** conferir a release nova, calcular os três
hashes, trocar as constantes, commitar. É o mesmo contrato do
`scripts/tor-bundle.lock.json`, só que as âncoras moram no código porque o
download é em runtime.

### 8.7 Revalidação (RF-38/39)

- No `setup()` do app, se `discord.reapply_on_start` e `mode != Off`: scan; se
  houver pasta `app-*` sem os arquivos e o Discord estiver fechado, reaplicar
  em silêncio; se estiver aberto, marcar `stale` e deixar a UI avisar.
- Em `set_config`, se as portas mudaram e `effective != Off`: reescrever só o
  `drover.ini` (a DLL lê no load, então o Discord precisa reiniciar) e sinalizar
  na UI: "reinicie o Discord para valer".

## 9. UI

Faixa **"Proxy no Discord"**, entre `Endpoints` e `Circuit`.

| Estado | O que mostra |
|---|---|
| Discord não encontrado | "Não encontrei o Discord instalado neste usuário." e nada mais |
| Componente ausente | O que é, tamanho (2,3 MB), origem (link do repo) e botão **Baixar pelo Tor** — desabilitado com "conecte o Tor primeiro" se não estiver conectado |
| Componente ok, proxy desligado | Sabores e pastas detectados, seletor HTTP (padrão) / SOCKS5, botão **Instalar** |
| Discord aberto durante a ação | Confirmação: "O Discord será fechado e reaberto." com checkbox "reabrir depois" |
| Instalado | Endereço em uso, pastas onde está, botão **Remover**, switch "reaplicar depois de atualizar o Discord" |
| `stale` | Aviso "as portas mudaram — reinicie o Discord" com botão **Reiniciar o Discord** |
| Erro | Mensagem do core, sem tratamento especial (padrão do app) |

Aviso permanente enquanto instalado (RF-37), no mesmo tom do `Disclaimer`:

> Chamadas de voz não passam pelo Tor. A voz do Discord é UDP, sai direto pela
> sua conexão e revela seu IP real. O proxy cobre mensagens, gateway e mídia.

E, antes da primeira instalação, uma linha sobre o Discord hostilizar IPs de
saída Tor (captcha, verificação por telefone, bloqueio de conta).

## 10. Dependências novas

| Crate | Para quê | Observação |
|---|---|---|
| `winreg` (windows) | ler `HKCU` na descoberta | ~alternativa: `windows-sys` cru, mais verboso |
| `sha2` | verificação do componente | |
| `zip` (`default-features = false`, `features = ["deflate"]`) | extrair dois arquivos | |
| `windows-sys` + feature `Win32_System_Diagnostics_ToolHelp` | snapshot de processos | a crate já está no projeto |

`reqwest` com `socks` já está no projeto. Nenhum plugin novo do Tauri para o
MVP (RF-41, se sair, pede `tauri-plugin-dialog`).

## 11. Testes

**Unitários** (13, rodam em qualquer plataforma — funções puras e diretórios
temporários; `cargo test --lib`):

- ordenação de versão: `app-1.0.9186` > `app-1.0.972` > `app-1.0.9`
- leitura da versão a partir do nome da pasta
- extração do diretório base a partir do valor de `shell\open\command`, e
  recusa de um comando que não aponte para uma pasta `app-*`
- geração e leitura do `drover.ini` (ida e volta, com comentário e espaços;
  `proxy =` vazio lê como "sem proxy")
- `stale`: instalado em `9080` contra config em `9081`; e pasta sem a nossa DLL
  não torna nada velho
- componente ausente ≠ componente corrompido; bytes errados nunca passam por
  `matches_dll`
- modo derivado do esquema da URL e URL derivada das portas configuradas
- reconhecimento dos três executáveis do Discord

Fora de teste automatizado, de propósito: a extração do zip (precisaria de um
zip forjado no repositório) e tudo que depende de registro e de processos vivos
— isso está no checklist manual.

**Compilação do lado Windows:** `cargo check` no Linux não cobre os blocos
`#[cfg(windows)]`. O registro (`winreg`) e o Toolhelp32/`TerminateProcess`
(`windows-sys`) foram verificados contra o alvo `x86_64-pc-windows-gnu`; a CI
compila o projeto inteiro no `windows-latest`.

**Manuais no Windows** (checklist de aceite):

1. Discord fechado → instalar → abrir Discord → mensagens carregam; em
   `check.torproject.org` pelo navegador interno do app o IP é de saída Tor
2. Discord aberto → instalar com "fechar e reabrir" → volta sozinho, funcionando
3. Remover → os três arquivos somem de todas as pastas `app-*`
4. Trocar a porta HTTP no Nika → `stale` aparece → reiniciar o Discord → volta ao normal
5. Forçar hash errado (apontar a URL para outro arquivo) → nada é instalado,
   nada fica em `%APPDATA%`
6. Instalar com o Tor desconectado → recusa com mensagem
7. Atualizar o Discord (nova `app-*`) → com `reapply_on_start`, reabrir o Nika
   com o Discord fechado reaplica
8. Sem Discord instalado → faixa some/avisa, nada quebra
9. Defender ativo: registrar se houve detecção — é risco conhecido, precisa de dado

## 12. Fases

| Fase | Conteúdo | Estimativa |
|---|---|---|
| **1 — Núcleo** | `discover.rs`, `process.rs`, `install.rs`, comandos, erros, config, testes unitários | 1,5–2 dias |
| **2 — Componente** | `component.rs` (download pelo Tor, hashes, manifesto), estados de erro | 1 dia |
| **3 — UI** | `DiscordProxy.tsx`, `useDiscord.ts`, tipos e ipc, avisos, estilos | 1 dia |
| **4 — Bordas** | revalidação (RF-38/39), item de bandeja (RF-40), checklist manual no Windows | 0,5–1 dia |

Total: **3,5 a 5 dias**. Fases 1 e 2 são independentes e podem ser paralelizadas.

## 13. Estado da implementação

Entregue e verificado (`cargo fmt`, `clippy -D warnings`, 29 testes, `npm run build`):

| Requisito | Onde |
|---|---|
| RF-27, RF-32 | `discord/discover.rs`, `Mode::proxy_url` |
| RF-28, RF-29 | `discord/process.rs` |
| RF-30, RF-31, RF-35 | `discord/install.rs`, `discord::scan` |
| RF-33, RF-34 | `discord/component.rs` |
| RF-36 | `commands::discord_install` / `discord_fetch_component` |
| RF-37 | `src/components/DiscordProxy.tsx` |
| RF-38 | `discord::sync_ports` + `commands::set_config` |
| RF-39 | `lib::reapply_discord`, no `setup()` |

Pendente (ambos `Could` no §5.1):

- **RF-40** — item na bandeja. O `tray.rs` hoje só reflete o estado do Tor;
  entrar aqui significa mais um eixo de estado no menu.
- **RF-41** — importar o componente de um `.zip` local. Pede
  `tauri-plugin-dialog` e só existe para quem não quer baixar pelo app.

## 14. Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Drover sem licença | Legal | D-02: não redistribuímos; pedir MIT ao autor. Se ele licenciar, o download some e o binário entra no instalador |
| Antivírus/SmartScreen sinalizando DLL ao lado do executável | Alto (adoção) | Assinar o instalador; explicar na UI e no README; nunca instalar sem clique |
| Componente muda de hash (nova release do drover) | Médio | Constantes fixadas; a falha é fechada (recusa), nunca "instala mesmo assim" |
| Discord muda o layout `app-*` | Médio | Descoberta degrada para "não encontrei"; nada é escrito no lugar errado |
| Update do Discord derruba a instalação | Baixo | A DLL se recopia via `CreateProcessW`; RF-39 é a rede de segurança |
| Usuário achar que está anônimo no Discord | Alto | RF-37 + aviso de conta/captcha |
| Matar o Discord com mensagem não enviada | Baixo | Confirmação explícita antes de fechar |

## 15. Questões em aberto

- Pedir licença ao autor do drover antes de implementar? Muda D-02 e simplifica tudo.
- Vale oferecer o modo sem DLL (lançar `Discord.exe --proxy-server=...` pelo
  Nika e reescrever o atalho) como alternativa de menor risco de antivírus?
  Cobre menos (não pega o updater nem o Discord aberto por outro caminho), mas
  não toca em nada dentro da pasta do Discord.
- `drover-packet.bin` só serve ao bypass de UDP, que não nos interessa. Vale
  copiá-lo mesmo assim (paridade com o drover) ou deixar de fora?
