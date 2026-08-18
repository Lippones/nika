# DLL própria do proxy do Discord — spec de implementação

**Status:** Implementado; gates de validação no Windows pendentes (§12, §13)
**Data:** 2026-08-17
**Substitui:** a decisão D-02 de [docs/discord-proxy.md](discord-proxy.md) (baixar o
binário de terceiro). O resto daquela spec — descoberta, processo, instalar/remover —
continua valendo sem mudança.
**Numeração:** RF-42 a RF-49, continuando o PRD.

---

## 1. Por que existir

A spec anterior instala, dentro da pasta do Discord, o `version.dll` do projeto
[discord-drover](https://github.com/hdrover/discord-drover). Isso resolve o
problema técnico, mas carrega dois defeitos que só somem se a DLL for **nossa**:

| Defeito com a DLL de terceiro | Consequência |
|---|---|
| O repo do drover não tem licença (all rights reserved) | Não podemos redistribuir o binário; por isso a spec anterior o baixa em runtime |
| O binário não é nosso | Não dá para **assinar** com nosso certificado (mitiga antivírus), nem garantir degradação segura se o hook falhar, nem fixar o comportamento |

Reescrever a DLL do zero em Rust — **sem copiar o código Delphi do drover, só
reimplementando o comportamento** — remove os dois. Comportamento não é protegido
por copyright; código é. Com a DLL própria: entra no instalador (some o download
e todo o `component.rs`), pode ser assinada, e podemos fazê-la falhar aberta (sem
proxy) em vez de derrubar o Discord.

> **Antes de investir aqui, a saída mais barata continua sendo pedir MIT ao autor
> do drover.** Um e-mail resolve a licença sem uma linha de código. Esta spec é o
> plano B — e o plano A se você quer controle total (assinatura, degradação).

## 2. O que a DLL faz (mínimo viável)

Um `version.dll` colocado ao lado do `Discord.exe`. O loader do Windows o carrega
no lugar do `version.dll` do System32 (search order: diretório do executável
primeiro), porque o processo do Discord importa `version.dll` estaticamente
(**gate G1, §12** — confirmar com `dumpbin /imports`).

Ao carregar, a DLL faz duas coisas:

1. **Reexporta as 17 funções do `version.dll` real**, encaminhando cada uma para
   o `version.dll` verdadeiro do System32. Sem isto, o import do Discord não
   resolve e o processo não sobe.
2. **Instala dois hooks inline** para injetar o proxy:

| Hook | Efeito | Cobertura |
|---|---|---|
| `GetCommandLineW` | acrescenta `--proxy-server=http://127.0.0.1:9080` | **primário** — o Chromium do Electron parseia isso e roteia ~toda a rede (gateway, API, mídia, updates de módulo) |
| `GetEnvironmentVariableW` | responde `http_proxy`/`https_proxy` | secundário — só o lado Node/libuv, e só libs que honram a variável |

**Fora de escopo, de propósito** (o que o drover faz e nós não):

- **Truque de UDP / `drover-packet.bin`** — serve para desbloquear voz em rede
  censurada. O Tor não transporta UDP; para nós é inútil e sai inteiro.
- **Reescrita `CONNECT`→SOCKS5 nos hooks de `send`/`recv`** — só existe no drover
  para o modo SOCKS5. Usamos o **HTTP tunnel** do Tor (`HTTPTunnelPort`, D-03 da
  spec anterior), que o Chromium fala nativamente. Zero hook de socket.
- **Hook de `CreateProcessW`** — no drover, recopia a DLL para pastas novas. No
  nosso desenho isso é papel do app (RF-39, `lib::reapply_discord`), não da DLL.

Cortar esses três deixa a DLL com **dois hooks e os reexports** — a fração
genuinamente simples do drover.

## 3. Decisões

| # | Decisão | Por quê |
|---|---|---|
| E-01 | Crate separado `src-tauri/discord-shim/`, `crate-type = ["cdylib"]`, `[lib] name = "version"` → gera `version.dll` | No MSVC não há prefixo `lib`; o nome do arquivo precisa ser exatamente `version.dll` |
| E-02 | **Reexport por função `#[unsafe(naked)]` + `naked_asm!`** com `jmp qword ptr [rip + ptr]` | O stub é agnóstico à assinatura: preserva registradores e salta direto, sem declarar os tipos das 17 funções. `#[no_mangle] pub` num cdylib já entra na export table com o nome exato — **dispensa `.def` e `build.rs`** |
| E-03 | **MSRV do shim = 1.88** (naked estabilizou em 1.88.0, 2025-06-26) | O app principal segue em 1.77; só o shim, que é Windows-only e não é consumido por ninguém, sobe |
| E-04 | Ponteiros reais resolvidos em `DllMain`/`DLL_PROCESS_ATTACH` via `GetSystemDirectoryW` + `LoadLibraryW(<sys>\version.dll)` + `GetProcAddress` | Caminho **absoluto** do System32; `LoadLibraryW("version.dll")` relativo carregaria a si mesmo (recursão) |
| E-05 | **Nunca** forwarders de `.def` (`EXPORTS Foo=version.Foo`) | Resolveriam de volta para a nossa própria `version.dll` (achada primeiro) → recursão infinita. É exatamente o que os stubs JMP contornam |
| E-06 | Engine de hook: **`retour = "0.3"`, `GenericDetour`** | Rust puro, roda em `stable`, faz length-disassembly + trampolim. `static_detour!` exige nightly — evitado |
| E-07 | Instalar os hooks **direto no `DllMain`**, não em thread | A DLL entra como import estático, carregada em `LdrpInitializeProcess` antes do entry point do EXE → o hook fica ativo antes de o Chromium ler a linha de comando. Uma thread criada no `DllMain` não é escalonada enquanto o loader lock estiver preso: perderia a corrida |
| E-08 | **Falhar aberto**: se resolução ou hook falharem, a DLL não injeta e deixa o Discord subir normal. Um reexport que não resolver aponta para um stub de fallback (`xor eax,eax; ret`), nunca fica em `jmp` para NULL | `panic = "abort"` no shim + nenhum `unwrap` no caminho de init: nada de unwind pela fronteira FFI (UB), nada de derrubar o Discord. O custo é anonimato (sem proxy), não estabilidade — ver §11 R-4 |
| E-09 | A DLL lê o proxy de um `nika-proxy.ini` ao lado dela | Mesmo mecanismo do drover (`drover.ini`), formato nosso. Sem proxy válido no ini → não injeta |
| E-10 | Só injeta `--proxy-server` quando o processo é `Discord*.exe` | O nome do executável (`GetModuleFileNameW`) filtra: cobre o main e todos os filhos `Discord.exe --type=...`, e não mexe em nada mais que carregue `version.dll` daquele diretório |
| E-11 | O shim vai como **recurso empacotado**, não baixado | É nosso: entra no instalador. `install.rs` copia da pasta de recursos, não de um download. `component.rs` (download) deixa de existir |

## 4. Mecânica de reexport (E-02)

Uma macro gera, para cada uma das 17 funções, um par (ponteiro global, stub
naked):

```rust
macro_rules! reexport {
    ($stub:ident => $slot:ident) => {
        static $slot: AtomicUsize = AtomicUsize::new(0);
        #[unsafe(naked)]
        #[no_mangle]
        pub extern "C" fn $stub() {
            // Salta para o version.dll real. Agnóstico à assinatura: não toca
            // em registrador nem stack, só desvia. `sym` resolve o endereço do
            // slot; o `jmp [slot]` lê o ponteiro que preenchemos em runtime.
            core::arch::naked_asm!("jmp qword ptr [rip + {ptr}]", ptr = sym $slot)
        }
    };
}
```

As 17: `GetFileVersionInfoA`, `GetFileVersionInfoByHandle`, `GetFileVersionInfoExA`,
`GetFileVersionInfoExW`, `GetFileVersionInfoSizeA`, `GetFileVersionInfoSizeExA`,
`GetFileVersionInfoSizeExW`, `GetFileVersionInfoSizeW`, `GetFileVersionInfoW`,
`VerFindFileA`, `VerFindFileW`, `VerInstallFileA`, `VerInstallFileW`,
`VerLanguageNameA`, `VerLanguageNameW`, `VerQueryValueA`, `VerQueryValueW`.

Exportar **todas** as 17, mesmo que o Discord só chame algumas: se o import pedir
uma que não exportamos, o loader falha e o processo não sobe. Custo de exportar a
mais: zero.

**Fallback (fail-open, E-08):** os 17 slots são preenchidos logo no início do
`DllMain`, antes de qualquer outra coisa. O que não resolver — ou tudo, se o
`version.dll` real nem carregar — aponta para um stub `xor eax, eax; ret`. Assim
um export ausente vira "retorna 0/NULL" (que o chamador trata) em vez de um
`jmp` para o endereço 0, que derrubaria o Discord. Sem isso, uma falha de
resolução seria fail-**closed** — o oposto do contrato.

## 5. Mecânica de hook (E-06, E-07, E-08)

```
DllMain(DLL_PROCESS_ATTACH):
  catch-all (sem panic; panic = abort):
    1. resolver os 17 ponteiros do version.dll real   ── se falhar, seguir mesmo assim
    2. decidir injeção: processo é Discord*.exe? ini tem proxy válido?
       └ não → não instala hook nenhum (transparente)
    3. ler a linha de comando real (GetCommandLineW do kernel32, antes do hook)
       montar UMA vez o buffer wide com " --proxy-server=<proxy>" + NUL
       guardar num static (OnceLock<Vec<u16>>)
    4. instalar GenericDetour em GetCommandLineW → devolve sempre esse buffer
    5. instalar GenericDetour em GetEnvironmentVariableW → http_proxy/https_proxy
  return TRUE  ── sempre, mesmo em falha: FALSE impediria o Discord de subir
```

Pontos que só parecem detalhe mas quebram tudo se errados:

- **O ponteiro de `GetCommandLineW` precisa ser estável e persistente.** O
  Chromium relê e cacheia o ponteiro. Devolver buffer de stack corrompe. Por isso
  o buffer é um `static`, montado uma vez, sempre o mesmo ponteiro, terminado em
  NUL.
- **Ler a linha original antes de instalar o hook**, chamando o `GetCommandLineW`
  real resolvido do kernel32 — assim o hook pode ser trivial (retorna o buffer) e
  não precisa de trampolim.
- **`GetEnvironmentVariableW` respeita o contrato de `nSize`**: buffer pequeno →
  retorna o tamanho necessário incluindo o NUL; suficiente → copia e retorna o
  comprimento sem o NUL. Para qualquer nome que não seja `http_proxy`/`https_proxy`,
  cai no trampolim (comportamento original).
- **Guardar os `GenericDetour` vivos** num `static`: se forem dropados, o hook se
  desinstala.
- **Loader lock:** instalar hook no `DllMain` é seguro aqui porque só se toca
  kernel32 (sempre carregada) com `VirtualProtect`+patch, sem `LoadLibrary`. O
  único `LoadLibrary` é o do `version.dll` do System32 (E-04); se algum dia der
  sinal de deadlock, mover a resolução dos ponteiros para init preguiçoso no
  primeiro uso (`Once`), mantendo só o hook no `DllMain`.

## 6. Integração com o app (E-11)

O que muda em relação a [docs/discord-proxy.md](discord-proxy.md):

| Antes (download) | Agora (shim próprio) |
|---|---|
| `component.rs` baixa o zip do drover pelo Tor, verifica hashes | `component.rs` só olha se o shim empacotado existe e confere o hash da cópia instalada contra o do shim empacotado |
| `discord_fetch_component` (comando) | removido — o shim vem no instalador |
| `drover.ini` + `version.dll` + `drover-packet.bin` | `nika-proxy.ini` + `version.dll` (sem packet) |
| `resources/` sem a DLL | `resources/discord/version.dll` (recurso empacotado) |
| Hash fixado no código (do drover) | Hash calculado em runtime a partir do shim empacotado — muda a cada build, então não pode ser constante |

Fluxo de recurso, espelhando o do `tor.exe`:

```
scripts/build-shim.ps1   (no Windows)
   └ cargo build -p version --release --target x86_64-pc-windows-msvc
   └ copia version.dll → src-tauri/resources/discord/version.dll
tauri.conf.json  →  "resources": [..., "resources/discord/*"]
install.rs  →  copia resources/discord/version.dll para cada app-*
```

Em dev/Linux, sem o shim compilado, o recurso não existe → `Component::Missing`,
igual ao que já acontece com `geoip` ausente. A UI mostra "componente não
incluído neste build" em vez de um botão de download.

`install.rs` também apaga, na desinstalação, os arquivos legados do drover
(`drover.ini`, `drover-packet.bin`) além dos nossos — limpeza para quem veio da
versão com download.

## 7. Requisitos

| ID | Requisito | Prioridade |
|---|---|---|
| RF-42 | DLL `version.dll` própria que reexporta as 17 funções do `version.dll` do sistema | Must |
| RF-43 | Injetar `--proxy-server=<http tunnel do Tor>` via hook de `GetCommandLineW`, só em processos `Discord*.exe` | Must |
| RF-44 | Injetar `http_proxy`/`https_proxy` via hook de `GetEnvironmentVariableW` | Should |
| RF-45 | Ler o endereço do proxy de `nika-proxy.ini` ao lado da DLL; sem proxy válido, não injetar | Must |
| RF-46 | Falhar aberto: qualquer erro de resolução/hook não pode derrubar o Discord | Must |
| RF-47 | Empacotar o shim como recurso e instalá-lo a partir dele (sem download de terceiro) | Must |
| RF-48 | `install.rs` grava `nika-proxy.ini` e reconhece a DLL instalada pelo hash do shim empacotado | Must |
| RF-49 | Assinar a DLL no pipeline de release (mitiga antivírus) | Should |

## 8. Layout do crate do shim

```
src-tauri/discord-shim/
  Cargo.toml        [lib] name="version", crate-type=["cdylib"], rust-version="1.88"
  src/lib.rs        DllMain, 17 reexports naked, resolução, hooks, leitura do ini
  .gitignore        target/
  README.md         como compilar e o que a DLL faz
```

`Cargo.toml` isola-se com um `[workspace]` vazio, para o `cargo` do app principal
nunca tentar compilá-lo junto.

## 9. Dependências do shim

| Crate | Para quê |
|---|---|
| `windows-sys` (LibraryLoader, Foundation, SystemInformation, SystemServices) | `LoadLibraryW`, `GetProcAddress`, `GetModuleHandleW`, `GetModuleFileNameW`, `GetSystemDirectoryW`, `DLL_PROCESS_ATTACH` |
| `retour = "0.3"` | hooks inline (`GenericDetour`) |

Sem `reqwest`, sem `zip` no shim. No app principal, a remoção do download tira
`zip` e a feature `stream` do `reqwest`; `sha2` permanece (hash do shim).

## 10. Testes

**No shim, sem Windows:** o que dá é `cargo check --target x86_64-pc-windows-gnu`
(type-check completo: naked, retour, windows-sys) e testes unitários das partes
puras — o parser do `nika-proxy.ini` e a montagem da linha de comando (dado uma
linha e um proxy, produzir a linha esperada, idempotente, terminada em NUL). O
hook, a resolução e o carregamento **não** rodam fora do Windows.

**No app principal:** os testes de `discord/*` que já existem seguem valendo;
`component.rs` ganha teste do "instalado = hash bate com o shim empacotado".

**Só no Windows (gates, §12):** carregar de fato dentro do Discord.

## 11. Riscos priorizados

| # | Risco | Prob. | Mitigação |
|---|---|---|---|
| R-1 | `Discord.exe` não importa `version.dll` neste build → a técnica não carrega | — | **Gate G1**: `dumpbin /imports` antes de confiar. Alvo alternativo documentado: `dbghelp.dll`, também import comum de Electron |
| R-2 | Antivírus/Defender/ASR: DLL de nome de sistema + hook inline em kernel32 = padrão de malware | Alta | **Assinar a DLL** (RF-49; possível porque é nossa — era impossível com o binário do drover). Documentar. Avaliar o modo sem DLL (`Discord.exe --proxy-server=` + atalho) como alternativa de menor atrito |
| R-3 | WebRTC/ICE vaza IP por UDP **antes** mesmo da voz (STUN, connectivity check) | Alta | `--proxy-server` não cobre UDP. Precisa de política de WebRTC (desabilitar UDP não-proxiado). Item de anonimato de primeira ordem, não só "voz" |
| R-4 | Hook falha → Discord sobe **sem** proxy (vaza IP real) em vez de crashar | Média | É a degradação escolhida (E-08). Se "nunca vazar" for inegociável, a política vira "hook falha → não deixar o Discord conectar" — decisão de produto, sinalizada, não resolvida aqui |
| R-5 | `Update.exe` (Squirrel) vive em `base_dir`, não em `app-*` → carrega o `version.dll` real, nenhum hook o alcança → o updater vaza o IP ao CDN | Média | Fora do alcance de qualquer hook desta DLL. Documentar honestamente. Correção real seria em outra camada (firewall/sistema) |
| R-6 | Control Flow Guard: `Discord.exe` é `/guard:cf`; o trampolim do `retour` pode ser rejeitado se não registrar via `SetProcessValidCallTargets` | Média | Verificar no Windows; se o `retour` não registrar, trocar de engine (minhook) ou registrar à mão. Gate G3 |
| R-7 | CET shadow stack (Win11 `/CETCOMPAT`) quebra trampolins que usem RET fora da shadow stack | Baixa-média | Trampolim por JMP evita; validar na matriz de teste |
| R-8 | `MITIGATION_FORCE_MICROSOFT_SIGNED_BINARIES` num processo filho do Chromium bloquearia a DLL não-Microsoft | Baixa | Canário de teste: confirmar que renderer/gpu sobem com a DLL presente. Se disparar em algum update do Electron, quebra o startup |

## 12. Gates de validação no Windows (bloqueadores)

Nenhum destes é verificável nesta máquina (Linux). **Antes de polir**, fechar:

- **G1 — import estático:** `dumpbin /imports Discord.exe` confirma `version.dll`?
  Se não, a abordagem inteira cai (ou muda de alvo para `dbghelp.dll`).
- **G2 — antivírus:** spike medindo detecção real (Defender + ASR ligados, Win10 e
  Win11), com a DLL assinada e não assinada.
- **G3 — CFG/CET:** o `retour` instala e o Discord roda em `Win11 /guard:cf` e
  `/CETCOMPAT`?
- **G4 — timing:** instrumentar que o hook está ativo antes da primeira leitura de
  `GetCommandLineW`.
- **G5 — cobertura/vazamento:** captura de pacotes provando que o TCP sai pelo Tor
  e medindo o que ainda vaza (WebRTC/UDP, `Update.exe`).

Matriz mínima: {Win10 21H2, Win11 23H2, Win11 24H2 com CET} × {Discord Stable,
Canary, PTB} × {Defender on/off}.

## 13. O que só o Windows confirma

Esta DLL foi escrita e **type-checada** (`cargo check` no alvo Windows), mas
**não foi carregada dentro de um processo real** — esta máquina é Linux. Tudo em
§12 é obrigatório antes de considerar a Fase 4 concluída. O código compila; que
ele hooka sem derrubar o Discord, só o teste no Windows prova.

## 14. Fases

| Fase | Conteúdo | Estimativa |
|---|---|---|
| **A — Shim** | crate, 17 reexports, resolução, hooks, leitura do ini, `cargo check` no alvo Windows, testes das partes puras | 3–5 dias |
| **B — Integração** | trocar `component.rs` de download para recurso; `install.rs` do shim; remover comando de download; ajustar UI; `build-shim.ps1`; empacotar recurso | 1–2 dias |
| **C — Gates** | G1 a G5 no Windows, matriz de teste, assinatura (RF-49) | 2–4 dias, depende de hardware Windows |

Fase C é a que decide se tudo isto vale — e é a única que não dá para adiantar
aqui.
