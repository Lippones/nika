# Saída própria para o Discord (túnel WireGuard) — spec de implementação

**Status:** Em implementação (Fase 5); ver §14
**Data:** 2026-10-19
**Depende de:** PRODUCT.md, `server/` (lado servidor)
**Numeração:** RF-50 em diante, continuando a numeração do PRD original (RF-01 a RF-49 eram do proxy Tor, removido em 2026-10-20). Decisões são T-xx.

---

## 1. Objetivo

Fazer **todo** o tráfego de rede dos processos do Discord — TCP e UDP — sair por
uma VM do próprio usuário (Oracle Cloud Always Free, fora do Brasil), sem tocar
no resto do tráfego do PC. Um clique no Nika liga e desliga.

Por que o proxy existente não basta: desde 17/08/2026 o Discord bloqueia Go Live
e vídeo no Brasil. O `--proxy-server` injetado pelo shim libera a interface (o
gateway vê um IP de fora), mas a mídia sai do `discord_voice.node` por **UDP
direto**, com o IP real, e o servidor de mídia recusa: `receiver count: 0`,
espectador em "carregando" para sempre. Proxy HTTP e Tor não carregam UDP. A
única coisa que fez o Go Live andar foi uma VPN de sistema — e é isso que este
documento reduz a "VPN só para o Discord".

Isto **não** anonimiza ninguém: a VM está na conta do usuário. Troca o IP de
saída do Discord por um IP fora do Brasil, e só.

## 2. Mecanismo

O Windows não roteia por aplicativo. O que dá para fazer é interceptar os
pacotes de um processo específico e levá-los por outro caminho:

```
Discord.exe ──pacote IP──▶ pilha do Windows ──▶ [WinDivert] ──▶ Nika
                                                               │ troca IP de origem (NAT)
                                                               │ encapsula (WireGuard, boringtun)
                                                               ▼
                                                      UDP ──▶ VM Oracle ──▶ Discord
                                                               ▲
Discord.exe ◀──pacote IP──  [WinDivert inject] ◀── Nika ◀── UDP (decapsula, desfaz o NAT)
```

1. **WinDivert** (driver assinado, LGPLv3) entrega ao Nika os pacotes de saída
   e permite injetar pacotes de entrada. Dois handles:
   - camada **SOCKET**, só escuta: eventos `BIND`/`CONNECT`/`CLOSE` com o PID.
     É daí que o Nika aprende quais portas locais pertencem a `Discord.exe`,
     `DiscordCanary.exe` e `DiscordPTB.exe` — antes do primeiro pacote sair.
   - camada **NETWORK**, saída: todo pacote TCP/UDP de saída passa pelo Nika.
     O que não é do Discord é devolvido na hora, intacto. O que é do Discord
     para um destino público é consumido e vai para o túnel.
2. **NAT 1:1 em espaço de usuário.** O pacote do Discord tem origem no IP da LAN
   (`192.168.x.y`). O Nika troca por o IP do túnel (`10.66.0.n`), corrige os
   checksums e guarda `(proto, porta local) → (IP da LAN, interface)` para
   desfazer no retorno. Não há pilha TCP no Nika: os dois lados da conexão
   continuam sendo o Discord e o servidor.
3. **WireGuard em espaço de usuário** com a crate `boringtun` (Cloudflare,
   BSD-3). Sem adaptador TUN, sem serviço: um socket UDP do Nika para o
   endpoint da VM. O perfil é o `.conf` que o `nika-peer` gera no servidor.
4. **Retorno:** o datagrama WireGuard chega, é decapsulado em um pacote IP com
   destino `10.66.0.n`, o Nika desfaz o NAT e injeta como pacote de entrada na
   interface de origem. Para o Discord, a resposta veio da rede.

## 3. Decisões

| # | Decisão | Por quê |
|---|---|---|
| T-01 | Interceptar por **processo**, via WinDivert, e não por faixa de IP (`AllowedIPs`) | Os servidores de mídia do Discord não têm CIDR estável; gateway/API são Cloudflare anycast. Uma lista de IPs quebraria toda semana |
| T-02 | **WinDivert**, não TUN (Wintun) + pilha própria | Com TUN todo o tráfego do PC passaria por uma pilha TCP em userspace (o não-objetivo do PRD §4). Com o WinDivert só o pacote do Discord é tocado; o resto é devolvido ao kernel sem alteração |
| T-03 | **boringtun** em processo, não o cliente oficial do WireGuard | O oficial roteia por adaptador, para todo o sistema. Precisamos de encapsular pacotes que nós escolhemos |
| T-04 | NAT 1:1 por `(proto, porta local)` | Um socket UDP do Discord fala com vários servidores; a chave pela porta local cobre todos e sobrevive à troca de servidor de voz |
| T-05 | Só destinos **públicos** IPv4 vão para o túnel | DNS para o roteador da LAN, mDNS, broadcast e RPC local precisam continuar saindo pela rede local |
| T-06 | IPv6 do Discord é **descartado** enquanto o túnel está ligado | A VM não tem IPv6 público e o servidor já recusa v6 no túnel. Descartar faz o Chromium cair para IPv4 (Happy Eyeballs). Injetar RST é melhoria futura |
| T-07 | `WinDivert.dll` carregada **dinamicamente** (`LoadLibrary`), sem import estático nem `.lib` | O Nika precisa subir, e compilar no Linux, mesmo sem o driver. Faltou a DLL → a faixa diz "componente ausente" |
| T-08 | Elevação: o Nika **se relança como administrador** quando o usuário liga o túnel | `WinDivertOpen` exige admin. Um serviço separado é a solução de longo prazo; relançar é o mínimo que funciona. A chave `Run` não eleva: com autostart o túnel só sobe depois de um clique |
| T-09 | O proxy Tor e o shim do Discord (`version.dll`) **saem do produto** | O túnel pega TCP e UDP; o shim só alcançava TCP e dependia do Tor. Removidos em 2026-10-20, ficam no histórico do git |
| T-10 | O perfil fica em arquivo próprio (`tunnel.conf`), não dentro do `config.json` | Contém a chave privada; separar deixa claro o que é segredo e permite importar/apagar sem tocar no resto |
| T-11 | Driver e DLL empacotados como recurso, com SHA-256 fixado em `scripts/windivert.lock.json` | Fonte oficial, bytes conferidos; o lock só muda por decisão de uma pessoa |
| T-12 | MSS de TCP clampado no Nika para `MTU − 40`, além do clamp no servidor | Pacote maior que o túnel não passa; clampar nos dois lados é barato e evita depender de ICMP |

## 4. O que o usuário precisa saber (vai na UI)

1. **Não é anonimato.** A VM é sua, em sua conta. O Discord passa a ver o IP da
   Oracle em vez do seu. Para quem observa a VM, é você.
2. **Precisa de administrador.** Ligar o túnel pede UAC uma vez por sessão do
   Nika. Com "iniciar com o Windows" o túnel só sobe depois do clique.
3. **Um driver é carregado** (`WinDivert64.sys`, assinado). Antivírus podem
   perguntar. O driver sai quando o Nika desliga o túnel.
4. **Voz e vídeo passam a sair pela VM.** É o objetivo — e também o custo:
   a latência da voz vira a latência até a Oracle e de lá até o Discord.

## 5. Escopo

### 5.1 Requisitos

| ID | Requisito | Prioridade |
|---|---|---|
| RF-50 | Importar um perfil WireGuard por colagem: o `.conf` do `nika-peer` ou o **convite de uma linha** (`nika://convite/<base64url do .conf>`), validando chaves, endereço e endpoint, com leitura ao vivo antes de gravar. Comentários `# Nika-Server:` e `# Nika-Peer:` dão o rótulo do servidor e o nome do convidado | Must |
| RF-51 | Ligar/desligar o túnel pela UI e pela bandeja; o estado real (handshake, bytes, fluxos) é emitido ao webview | Must |
| RF-52 | Interceptar **só** pacotes dos processos do Discord (3 sabores), por PID, aprendidos na camada SOCKET antes do primeiro pacote | Must |
| RF-53 | NAT 1:1 de origem com correção de checksum IPv4/TCP/UDP; desfazer no retorno e injetar na interface de origem | Must |
| RF-54 | Destinos privados, link-local, multicast, broadcast e loopback nunca entram no túnel (T-05) | Must |
| RF-55 | Clamp de MSS em SYN de saída para `MTU − 40` (T-12) | Must |
| RF-56 | Detectar falta de elevação e oferecer "reabrir como administrador", preservando a sessão (sem segunda instância) | Must |
| RF-57 | Driver/DLL empacotados a partir do zip oficial com SHA-256 fixado; faltando, a faixa informa (T-07, T-11) | Must |
| RF-58 | Desligar remove todos os fluxos, fecha os handles e descarrega o driver; matar o Nika não deixa handle aberto (o kernel fecha ao sair do processo) | Must |
| RF-59 | Keepalive e re-handshake do WireGuard sem intervenção; perda de link mostra "sem resposta do servidor" com o tempo desde o último handshake | Should |
| RF-60 | Apagar o perfil (chave privada) do disco | Should |
| RF-61 | Pacote IPv4 maior que o MTU com DF: responder ICMP "fragmentation needed" ao Discord | Could |
| RF-62 | IPv6 TCP SYN do Discord: injetar RST em vez de descartar (T-06) | Could |
| RF-63 | Elevação por serviço Windows em vez de relançar o app (T-08) | Could |

### 5.2 Fora de escopo

| Item | Motivo |
|---|---|
| Rotear outros apps além do Discord | Fora do propósito (PRODUCT.md); o desenho permite, a UI não oferece |
| IPv6 pelo túnel | A VM não tem v6 público e o servidor recusa |
| Verificar IP de saída pelo túnel | Exigiria pilha TCP própria no Nika. O IP de saída é o do endpoint (NAT 1:1 da Oracle), e a UI mostra isso |
| Criar a VM a partir do Nika | Oracle exige console/Cloud Shell autenticado; `server/oracle-create.sh` roda lá |

## 6. Arquitetura

```
src-tauri/src/tunnel/
  mod.rs        tipos públicos (TunnelStatus, Phase, Component), TunnelStore, start/stop
  profile.rs    parse e validação do .conf WireGuard  (puro, testado no Linux)
  packet.rs     IPv4/TCP/UDP: parse, NAT de origem/destino, checksum, MSS, destino público (puro)
  wg.rs         Tunnel: boringtun + socket UDP + timers, canais de pacotes IP (puro)
  engine.rs     [windows] orquestra WinDivert ⇄ NAT ⇄ wg; tabela de fluxos
  divert.rs     [windows] FFI dinâmico ao WinDivert.dll (Open/Recv/Send/Shutdown/Close)
  owner.rs      [windows] PID → nome do executável; porta → PID (GetExtended*Table), fallback
  elevation.rs  [windows] token elevado? relançar com runas e esperar o PID antigo sair
  fallback.rs   [não-windows] engine que devolve Error::TunnelUnsupported
```

Threads e tarefas com o túnel ligado:

| Quem | Faz |
|---|---|
| thread `divert-net` | `WinDivertRecv` em loop. Não-Discord → `WinDivertSend` de volta. Discord → NAT → `outbound_tx` |
| thread `divert-sock` | eventos SOCKET: mantém `ports: (proto, porta) → PID` dos processos do Discord |
| tarefa `wg` (tokio) | `outbound_rx` → `encapsulate` → UDP; UDP → `decapsulate` → `inbound_tx`; `update_timers` a cada 250 ms |
| thread `divert-inject` | `inbound_rx` → desfaz NAT → `WinDivertSend` (entrada, interface de origem) |

### 6.1 Layout em disco

```
%APPDATA%\dev.nika.tortray\
  tunnel.conf                  perfil importado (chave privada), modo 600-equivalente
resources\windivert\
  WinDivert.dll                carregada em runtime (T-07)
  WinDivert64.sys              o driver; a DLL o instala a partir do próprio diretório
```

## 7. Contratos

### 7.1 Tipos (`tunnel/mod.rs`)

```rust
pub enum Phase { Off, Starting, Handshaking, Up, Failed }

pub enum Component { Missing, Ready }        // WinDivert.dll + .sys presentes

pub struct TunnelStatus {
    pub phase: Phase,
    pub component: Component,
    pub elevated: bool,                       // processo atual é admin
    pub profile: Option<ProfileSummary>,      // sem chave privada
    pub endpoint: Option<String>,             // "ip:porta" = IP de saída visto pelo Discord
    pub last_handshake_secs: Option<u64>,
    pub tx_bytes: u64, pub rx_bytes: u64,
    pub flows: u32,                           // portas do Discord em tabela
    pub error: Option<String>,
}

pub struct ProfileSummary {
    pub address: String, pub endpoint: String, pub peer_public_key: String, pub mtu: u16,
    pub server_label: Option<String>,         // "# Nika-Server: Filipe · Santiago"
    pub peer_name: Option<String>,            // "# Nika-Peer: joao"
}
```

### 7.2 Config (`config.rs`)

```rust
pub struct TunnelConfig {
    /// Subir o túnel ao abrir o Nika, se houver perfil e o processo for elevado.
    pub auto_start: bool,
}
```

### 7.3 Comandos

| Comando | Faz |
|---|---|
| `tunnel_status()` | estado atual |
| `tunnel_preview(text)` | decodifica e valida sem gravar: o resumo que a UI mostra enquanto a pessoa cola |
| `tunnel_import(text)` | aceita `.conf` ou convite de uma linha; valida e grava `tunnel.conf` decodificado; devolve estado |
| `tunnel_forget()` | apaga `tunnel.conf`; exige túnel desligado |
| `tunnel_start()` | sobe; `Error::NeedsElevation` se não for admin; `TunnelComponentMissing` se faltar a DLL |
| `tunnel_stop()` | desce |
| `relaunch_elevated()` | RF-56 |

Evento: `nika://tunnel` com `TunnelStatus`.

### 7.4 Erros novos

```rust
#[error("o perfil WireGuard é inválido: {0}")]            TunnelProfile(String),
#[error("nenhum perfil WireGuard importado")]              TunnelNoProfile,
#[error("o túnel precisa de permissão de administrador")]  NeedsElevation,
#[error("o componente do túnel (WinDivert) não foi incluído neste build")] TunnelComponentMissing,
#[error("o túnel só funciona no Windows")]                 TunnelUnsupported,
#[error("WinDivert: {0}")]                                 Divert(String),
```

## 8. Algoritmos

### 8.1 Classificar um pacote de saída (`engine.rs`)

```
pacote, addr ← WinDivertRecv
se addr.loopback ou pacote não é IPv4/IPv6 TCP/UDP → devolver intacto
porta ← porta de origem; proto ← TCP|UDP
dono ← ports[(proto, porta)]                          # camada SOCKET
se dono ausente → dono ← owner::lookup(proto, porta)  # GetExtended*Table, cache 2 s
se dono não é Discord → devolver intacto
se IPv6 → descartar (T-06)
se destino não é público → devolver intacto (T-05)
se TCP SYN → clamp MSS (T-12)
nat[(proto, porta)] ← (ip de origem, IfIdx, SubIfIdx)
origem ← ip do túnel; corrigir checksums
outbound_tx.send(pacote)
```

### 8.2 Retorno (`engine.rs`)

```
pacote ← inbound_rx                      # já decapsulado, destino = ip do túnel
(ip, if, subif) ← nat[(proto, porta de destino)]  ou descartar
destino ← ip; corrigir checksums
WinDivertSend(pacote, addr{Outbound=0, IfIdx=if, SubIfIdx=subif})
```

### 8.3 Correção de checksum (`packet.rs`)

Troca de um endereço de 32 bits: atualização incremental (RFC 1624) no header
IPv4 e no pseudo-header de TCP/UDP. UDP com checksum 0 continua 0. Mudança do
MSS (2 bytes) usa a mesma rotina sobre o checksum TCP.

### 8.4 WireGuard (`wg.rs`)

- `encapsulate(ip, buf)`: `WriteToNetwork` → `udp.send_to(endpoint)`. Sem sessão,
  o boringtun enfileira o pacote e devolve o handshake init; o pacote sai no
  handshake concluído.
- Datagrama recebido: `decapsulate(Some(src), dgram, buf)`; `WriteToNetwork` →
  enviar e repetir com datagrama vazio até `Done`; `WriteToTunnelV4` →
  `inbound_tx`.
- A cada 250 ms: `update_timers(buf)`; `WriteToNetwork` → enviar. Keepalive
  persistente vem do perfil (`PersistentKeepalive = 25`).
- `stats()` alimenta `last_handshake_secs`, `tx_bytes`, `rx_bytes`.

### 8.5 Elevação (`elevation.rs`)

`is_elevated()`: `OpenProcessToken` + `GetTokenInformation(TokenElevation)`.
`relaunch()`: `ShellExecuteW(runas, exe, "--after-pid <pid atual> --tunnel")`.
Sucesso (> 32) → o processo atual sai. O novo, em `main`, espera o PID antigo
terminar **antes** de inicializar o Tauri — senão o plugin de instância única
o mataria. `--tunnel` liga o túnel assim que o app sobe.

## 9. UI

Decisão de produto (PRODUCT.md): quem convida gerencia no terminal; quem é
convidado nunca vê terminal, chave ou IP além do que a faixa mostra. Sem aba:
o bilhete é uma coluna, e o convite entra em dois lugares.

**Onboarding** (`Onboarding.tsx`): a tela de boas-vindas oferece "Tenho um
convite" como ação principal e "Ainda não". O passo do convite tem o campo compartilhado `InviteInput` (cola
o `nika://convite/…` ou o `.conf`; o resumo "Convite para joao · Servidor
Filipe · Santiago" aparece ao vivo, via `tunnel_preview`), três fatos (só
Discord; admin; IP) e "Usar convite e ligar" — que grava, conclui o onboarding
e, sem elevação, reabre como administrador com `--tunnel`.

**Talão** (`Stub.tsx`): o estado da saída em letras grandes (desligada ·
ligando · no ar · falha · sem convite) e a ação: "Ligar", "Desligar" ou
"Reabrir como administrador".

**Faixa "Convite"** (`Tunnel.tsx`), logo abaixo do talão:

- sem convite: `InviteInput` + "Usar convite" (habilita só com convite lido)
- com convite: Servidor (rótulo) · Endereço · Para (quem) · Handshake ·
  Tráfego · Fluxos; "Esquecer convite" e o interruptor "Ligar ao abrir"
- sem resposta há mais de 3 min: "o servidor pode estar desligado, ou o seu
  convite foi revogado; fale com quem te convidou" (o WireGuard não distingue)
- sem elevação: aviso + "Reabrir como administrador"
- componente ausente: aviso com a receita (`fetch-windivert`)
- aviso permanente: "não é anonimato; o Discord vê o IP do servidor de quem te
  convidou; nada mais do PC passa por ele"

Bandeja: "Ligar saída do Discord"/"Desligar saída do Discord".

## 10. Dependências novas

| Crate | Para quê | Licença |
|---|---|---|
| `boringtun` 0.7 (sem features) | protocolo WireGuard | BSD-3 |
| `base64` 0.22 | chaves do `.conf` | MIT/Apache |
| `windows-sys` features `Win32_NetworkManagement_IpHelper`, `Win32_Networking_WinSock`, `Win32_UI_Shell`, `Win32_System_Threading`, `Win32_Security` | tabelas de portas, `ShellExecuteW`, token | MIT/Apache |

Binários: WinDivert 2.2.2 (`WinDivert.dll`, `WinDivert64.sys`), LGPLv3,
carregado dinamicamente — sem link estático, a LGPL é cumprida.

## 11. Testes

Rodam no Linux (`cargo test`):

- `profile`: perfil do `nika-peer` parseia; chave com tamanho errado, endpoint
  sem porta e `Address` sem IPv4 são recusados; a chave privada não aparece em
  `Debug`/`Serialize`.
- `packet`: checksum IPv4 recalculado bate com o cálculo completo após a troca
  de origem; TCP e UDP idem; UDP com checksum 0 segue 0; MSS clampado só em
  SYN e só quando maior; destinos privados/multicast/broadcast não são públicos.
- `wg`: dois `Tunnel` em `127.0.0.1` fazem handshake e um pacote IPv4 vai e
  volta (teste de integração, tokio).

Só no Windows (gates em §13): tudo que toca o driver.

## 12. Riscos

| Risco | Mitigação |
|---|---|
| Antivírus barra o driver ou a DLL | Binários oficiais, assinados; a UI explica o que é. Documentar exclusão |
| Primeiro pacote UDP sai antes do evento SOCKET | Fallback por `GetExtendedUdpTable` no pacote de porta desconhecida (§8.1) |
| Todo pacote de saída do PC passa pelo Nika enquanto o túnel está ligado | Devolução imediata sem cópia extra; `WinDivertRecvEx` em lote se o throughput pedir. Medir no gate 4 |
| Discord muda o IP de descoberta UDP e o servidor compara com o IP do gateway | É o comportamento esperado: os dois saem pela VM |
| Oracle recicla a VM (Always Free ocioso) | Fora do Nika; `nika-peer list` mostra o handshake; documentar |

## 13. Gates de validação no Windows (bloqueadores)

1. `WinDivertOpen` abre as duas camadas elevado e falha com `ERROR_ACCESS_DENIED`
   sem elevação (é o que vira `NeedsElevation`).
2. Com o túnel ligado, `curl https://1.1.1.1/cdn-cgi/trace` fora do Discord
   mostra o IP real; no DevTools do Discord, `fetch` do mesmo URL mostra o IP da
   VM.
3. Go Live: `%APPDATA%\discord\logs\discord-webrtc_0` mostra `receiver count: 1`
   e `frames encoded` > 0 com espectador em outra rede.
4. Download fora do Discord a 100 Mbps+ sem queda perceptível com o túnel ligado.
5. Desligar o túnel: `sc query WinDivert` sem o serviço; nenhum fluxo preso.
6. Matar o Nika pelo Gerenciador de Tarefas: o Discord volta a sair direto, sem
   pacote preso (o kernel descarta a fila ao fechar o handle).

## 14. Fases

- **5a** — perfil, pacotes, WireGuard (puros, testados no Linux) ✓
- **5b** — FFI WinDivert, engine, elevação, comandos, UI, bundle do driver ✓
  (escrito e type-checado no alvo Windows; nunca executado num Windows real)
- **5c** — gates §13 no Windows; ajustes ← **próximo passo**
- **5d** — RF-61/62/63 conforme a necessidade

Estado em 2026-10-19: código completo em `src-tauri/src/tunnel/`, UI em
`src/components/Tunnel.tsx`, scripts em `scripts/fetch-windivert.*`. Tudo que
toca o driver é teoria até o gate 1 passar. Pontos a confirmar primeiro no
Windows: ordem de bytes das portas nos eventos SOCKET (`divert.rs`,
`Address::socket`), e se o `IfIdx` capturado na saída serve para injetar a
resposta na entrada (é o que a documentação do WinDivert descreve).

## 15. Questões em aberto

- Serviço Windows para elevação persistente (RF-63) vale o custo de instalar
  um serviço no `currentUser` NSIS?
- Oferecer "ligar o túnel ao abrir o Discord" (observar processo) em vez de
  deixá-lo sempre ligado?
