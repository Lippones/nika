# PRD — Tor Tray (nome provisório)

**Status:** Draft
**Data:** 2026-08-17
**Autor:** Filipe Vieira

---

## 1. Problema

Hoje, para ter um proxy Tor disponível no Windows, é preciso abrir o Tor Browser e
mantê-lo aberto — ele é quem sobe o daemon e expõe o SOCKS5 em `127.0.0.1:9150`.
Isso é um custo desnecessário quando o objetivo não é navegar, e sim ter um proxy
Tor disponível para outros aplicativos (clientes HTTP, scripts, ferramentas de
terminal, apps que aceitam proxy).

## 2. Solução

Um app de bandeja (system tray) para Windows, feito em Tauri, que empacota o
**Tor Expert Bundle** e gerencia o ciclo de vida do `tor.exe`. Ele sobe junto com o
Windows, conecta na rede Tor em background e deixa um **proxy SOCKS5 e um proxy
HTTP(S) escutando em localhost**, prontos para qualquer app apontar.

Sem navegador. Sem driver. Sem privilégio de administrador.

## 3. Objetivos

- Ter um proxy Tor disponível ~10s após o login do Windows, sem interação
- Expor o estado da conexão de forma clara (bootstrap %, circuito, IP de saída)
- Permitir trocar de identidade (novo circuito) com um clique
- Consumo de recursos baixo e previsível
- Instalação e uso sem privilégio de administrador

## 4. Não-objetivos (explícito)

| Fora de escopo | Motivo |
|---|---|
| Rotear todo o tráfego do PC de forma transparente | Exige adaptador TUN + driver; alto custo, alta chance de vazamento de DNS |
| Alterar o proxy de sistema do Windows (WinINET) | Cobertura parcial e imprevisível entre apps; fonte comum de vazamento |
| Navegador embutido | Anonimato de navegação depende do hardening do Tor Browser, não do daemon |
| Rodar relay / exit node | Escopo e responsabilidade totalmente diferentes |
| Hidden services (onion services) | Avaliar em fase futura |
| macOS / Linux | Fase 2 |

> **Aviso que precisa estar visível na UI:** este app troca o seu IP, ele **não** te
> torna anônimo. Fingerprinting de browser, vazamento de WebRTC, telemetria de app e
> contas logadas continuam te identificando. Para navegação anônima, use o Tor Browser.

## 5. Usuário-alvo

Desenvolvedor / usuário técnico no Windows que quer um endpoint SOCKS5 Tor sempre
disponível para apontar ferramentas específicas — não alguém buscando anonimato
completo de sistema.

## 6. Requisitos funcionais

### 6.1 Ciclo de vida do Tor

| ID | Requisito | Prioridade |
|---|---|---|
| RF-01 | Iniciar o `tor.exe` como processo filho (sidecar), com `torrc` gerado pelo app | Must |
| RF-02 | Encerrar o `tor.exe` ao fechar o app, sem deixar processo órfão (inclusive em crash do app) | Must |
| RF-03 | Detectar queda do `tor.exe` e reiniciar automaticamente com backoff exponencial (1s → 30s, máx. 5 tentativas) | Must |
| RF-04 | Impedir múltiplas instâncias do app (single instance) | Must |
| RF-05 | Detectar porta ocupada (ex.: Tor Browser já usando 9050) e falhar com mensagem clara, sugerindo porta alternativa | Must |

### 6.2 Proxy

| ID | Requisito | Prioridade |
|---|---|---|
| RF-06 | Expor SOCKS5 em `127.0.0.1:9050` (porta configurável) | Must |
| RF-07 | Expor HTTP CONNECT tunnel em `127.0.0.1:9080` via `HTTPTunnelPort` — muitos apps não falam SOCKS5, mas falam HTTP proxy | Must |
| RF-08 | Botão "copiar endereço do proxy" na UI (formato `socks5://127.0.0.1:9050` e `http://127.0.0.1:9080`) | Should |
| RF-09 | Expor `DNSPort` opcional para resolução DNS via Tor | Could |
| RF-10 | Bind **exclusivamente** em `127.0.0.1`, nunca `0.0.0.0` | Must |

### 6.3 Estado e controle

| ID | Requisito | Prioridade |
|---|---|---|
| RF-11 | Conectar no ControlPort e assinar eventos de bootstrap; exibir progresso em % e a fase atual | Must |
| RF-12 | Autenticação no ControlPort via **cookie** (`CookieAuthentication 1`), nunca senha hardcoded no binário | Must |
| RF-13 | Botão "Nova identidade" → `SIGNAL NEWNYM` | Must |
| RF-14 | Exibir o circuito atual (nós e países) via `GETINFO circuit-status` | Should |
| RF-15 | Verificar IP de saída sob demanda (requisição via o próprio proxy, ex.: `check.torproject.org/api/ip`) — nunca automático no boot | Should |
| RF-16 | Exibir bytes enviados/recebidos (`GETINFO traffic/read`, `traffic/written`) | Could |
| RF-17 | Painel de log do Tor (últimas N linhas de notice), útil para diagnóstico | Should |

### 6.4 Tray e inicialização

| ID | Requisito | Prioridade |
|---|---|---|
| RF-18 | Ícone na bandeja com estado visual distinto: desconectado / conectando / conectado / erro | Must |
| RF-19 | Menu de contexto: Abrir, Conectar/Desconectar, Nova identidade, Copiar proxy, Sair | Must |
| RF-20 | Iniciar com o Windows (registro `HKCU\...\Run` — sem admin), togglável na UI | Must |
| RF-21 | Iniciar minimizado na bandeja quando lançado pelo autostart | Must |
| RF-22 | Fechar a janela (X) esconde na bandeja; sair só pelo menu do tray | Must |
| RF-23 | Opção "conectar automaticamente ao iniciar" (default: ligado) | Should |

### 6.5 Configuração

| ID | Requisito | Prioridade |
|---|---|---|
| RF-24 | Persistir configuração do usuário em disco (portas, autostart, autoconnect) | Must |
| RF-25 | Suporte a bridges obfs4 / snowflake para redes que bloqueiam Tor (`lyrebird.exe` já vem no bundle) | Should |
| RF-26 | Escolher país de saída (`ExitNodes {xx}` + `StrictNodes 1`), com aviso de que reduz anonimato | Could |

## 7. Requisitos não-funcionais

- **Memória:** app + tor abaixo de ~150 MB RSS em regime
- **CPU:** desprezível em idle (< 1%)
- **Tempo até proxy pronto:** < 15s em rede normal, contado do início do processo
- **Sem admin:** instalação e execução em contexto de usuário
- **Tamanho do instalador:** < 40 MB
- **Offline-first:** o app não faz nenhuma chamada de rede própria fora do Tor

## 8. Arquitetura

```
┌─────────────────────────────────────────┐
│  Tauri App (Rust core + WebView UI)     │
│                                         │
│  ┌───────────────┐   ┌────────────────┐ │
│  │ Supervisor    │   │ Control Client │ │
│  │ (spawn/kill/  │   │ (protocolo do  │ │
│  │  restart)     │   │  ControlPort)  │ │
│  └───────┬───────┘   └───────┬────────┘ │
│          │                   │          │
└──────────┼───────────────────┼──────────┘
           │ spawn             │ TCP 127.0.0.1:9051
           ▼                   ▼
      ┌──────────────────────────────┐
      │  tor.exe (sidecar)           │
      │  Tor Expert Bundle           │
      └──────┬───────────────┬───────┘
             │ 9050          │ 9080
          SOCKS5          HTTP CONNECT
             │               │
             ▼               ▼
        apps do usuário apontam aqui
```

### 8.1 Stack

- **Tauri v2** (Rust + WebView2 — WebView2 já vem no Windows 11)
- **Frontend:** React + Vite + TypeScript (UI é pequena; qualquer stack serve)
- **Plugins:** `tauri-plugin-autostart`, `tauri-plugin-single-instance`, `tauri-plugin-store`, `tauri-plugin-shell`
- **Binário externo:** `tor.exe` do Tor Expert Bundle, declarado como `externalBin`
  (nome precisa do sufixo de target triple: `tor-x86_64-pc-windows-msvc.exe`)

### 8.2 torrc gerado em runtime

Gerado no `AppData` do usuário a cada start, a partir da config:

```
SocksPort 127.0.0.1:9050
HTTPTunnelPort 127.0.0.1:9080
ControlPort 127.0.0.1:9051
CookieAuthentication 1
DataDirectory <AppData>\tor-tray\data
GeoIPFile <resources>\geoip
GeoIPv6File <resources>\geoip6
ClientOnly 1
AvoidDiskWrites 1
Log notice stdout
```

### 8.3 Protocolo de controle (referência)

| Ação | Comando |
|---|---|
| Autenticar | `AUTHENTICATE <cookie-hex>` (ou `AUTHCHALLENGE SAFECOOKIE`) |
| Progresso de conexão | `SETEVENTS STATUS_CLIENT` → evento `650 STATUS_CLIENT ... BOOTSTRAP PROGRESS=n` |
| Estado atual | `GETINFO status/bootstrap-phase` |
| Nova identidade | `SIGNAL NEWNYM` |
| Circuitos | `GETINFO circuit-status` |
| Tráfego | `GETINFO traffic/read` / `traffic/written` |
| Desligar | `SIGNAL HALT` |

## 9. Fluxos principais

**Boot do Windows**
1. Windows executa o app via chave Run (HKCU)
2. App inicia oculto na bandeja, ícone em "conectando"
3. Gera `torrc`, faz spawn do `tor.exe`
4. Conecta no ControlPort, lê cookie, autentica, assina eventos
5. Bootstrap chega a 100% → ícone vira "conectado", notificação opcional
6. Proxy disponível em 9050 / 9080

**Nova identidade**
1. Usuário clica no botão (UI ou tray)
2. `SIGNAL NEWNYM`
3. UI mostra feedback e recarrega o circuito
4. Nota: conexões já abertas continuam no circuito antigo — a UI precisa deixar isso claro

**Falha de conexão**
1. Bootstrap trava (ex.: Tor bloqueado na rede)
2. Após timeout (60s), ícone vira "erro" e a UI sugere ativar bridges
3. Log fica acessível para diagnóstico

## 10. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| Processo `tor.exe` órfão após crash do app | Alto — porta presa, processo zumbi | Windows **Job Object** com `KILL_ON_JOB_CLOSE`; o SO mata o filho junto com o pai |
| Binário Tor adulterado | Crítico | Baixar só de torproject.org, **verificar assinatura GPG**, fixar checksum no build e validar em CI |
| Tor desatualizado no bundle | Alto — falhas de segurança conhecidas | Checagem de versão + processo documentado de atualização; auto-update do binário é fase 2 |
| Porta 9050 em uso pelo Tor Browser | Médio | Detectar no start e propor porta alternativa (ex.: 9052) |
| Antivírus / SmartScreen marcando o app | Médio | Assinar o instalador com certificado de code signing |
| Usuário assumir que está anônimo | Alto | Disclaimer permanente na UI, não só no onboarding |
| Rede do usuário bloqueia Tor | Médio | Suporte a bridges obfs4 (RF-25) |
| ControlPort exposto | Crítico | Bind só em 127.0.0.1 + cookie auth; nunca `0.0.0.0`, nunca senha no código |

## 11. Fases

**Fase 1 — MVP**
RF-01 a RF-07, RF-10 a RF-13, RF-18 a RF-24.
Resultado: tray sobe com o Windows, conecta, mostra %, expõe SOCKS5/HTTP, nova identidade.

**Fase 2 — Usabilidade e resiliência**
RF-08, RF-14, RF-15, RF-17, RF-25. Assinatura do instalador. Auto-update do binário Tor.
j
**Fase 3 — Avançado**
RF-09, RF-16, RF-26. Perfis de configuração. Avaliar macOS/Linux e onion services.

**Fase 4 — Proxy no Discord**
RF-27 a RF-41: instalar/remover o proxy no app do Discord a partir do Nika,
detectando pastas, porta e processo em execução. Spec completa em
[docs/discord-proxy.md](docs/discord-proxy.md). Depende de decisão sobre a
licença do componente de terceiro (`discord-drover`).

## 12. Critérios de sucesso do MVP

- Reboot do Windows → proxy funcional sem nenhum clique, em < 15s
- `curl --socks5-hostname 127.0.0.1:9050 https://check.torproject.org/api/ip` retorna `IsTor: true`
- Matar o app pelo Gerenciador de Tarefas não deixa `tor.exe` rodando
- 24h ligado sem vazamento de memória ou queda não recuperada

## 13. Questões em aberto
j
- Nome e identidade visual do app
- Verificação de IP de saída: automática no boot ou só sob demanda? (automática cria uma requisição previsível a cada start — preferência atual: sob demanda)
- Vale expor um `.pac` ou perfil pronto para facilitar configuração dos apps clientes?
- Distribuição: GitHub Releases apenas, ou também winget?