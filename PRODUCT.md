# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Dono da saída** (o Filipe, e quem mais seguir `docs/oracle-exit.md`): sobe uma VM na Oracle Cloud, gerencia quem tem acesso pelo terminal (`nika-peer add|remove|list` via SSH) e usa o Nika no próprio PC para o Go Live.
- **Convidados**: 3 a 10 amigos leigos, no Windows, que só querem que o Go Live e a voz do Discord voltem a funcionar. Recebem um convite do dono por mensagem (Discord/WhatsApp), colam no Nika e seguem. Nenhum deles usa terminal.
- **Usuário do proxy Tor** (o público original do Nika): aponta apps para o SOCKS5/HTTP, ou instala o proxy no Discord para texto.

## Product Purpose

Nika é um app de bandeja do Windows que troca o IP de saída de aplicativos. Começou como proxy Tor; desde o bloqueio de Go Live e vídeo do Discord no Brasil (agosto de 2026), passou a oferecer uma **saída própria só para o Discord**: um túnel WireGuard até uma VM do próprio dono, pelo qual passam TCP e UDP dos processos do Discord, e nada mais do PC. Sucesso: o Go Live anda (`receiver count: 1`) para o dono e para cada convidado, sem ninguém configurar rota, DNS ou firewall.

## Positioning

Só o tráfego do Discord sai pela VM, por processo, sem adaptador de rede nem VPN de sistema. Uma VPN comum roteia o PC inteiro; o proxy do Discord sozinho não leva UDP. O Nika leva as duas coisas (TCP e UDP) e nada além delas.

## Operating Context

- Convite = o `.conf` WireGuard que `nika-peer add <nome>` imprime na VM. Um por pessoa. É o único passe: a VM ignora quem não tem chave. Revogação imediata com `nika-peer remove`.
- Entrega do convite: texto copiado numa DM. Não há canal dentro do app para isso.
- Ligar o túnel exige administrador (driver WinDivert). O Nika reabre pedindo UAC.
- A VM é grátis (Oracle Always Free) e pequena: poucos usuários, prioridade para o Go Live.
- Tudo que sai da VM sai no IP e na conta do dono: convite só para gente de confiança, e só para uso no Nika.

## Capabilities and Constraints

- Proxy Tor (SOCKS5/HTTP) com bandeja, autostart, nova identidade, circuito.
- Proxy no Discord via `version.dll` própria (TCP só).
- Túnel do Discord: importar perfil, ligar/desligar, estado (handshake, tráfego, fluxos), esquecer perfil, reabrir elevado, ligar ao abrir. Spec em `docs/discord-tunnel.md`.
- Gestão de convidados fica fora do app, no terminal (decisão de produto: menos segredo e menos superfície no app).
- Não é anonimato. A UI diz isso sempre.
- Repositório público; o código não pode presumir uma VM específica.

## Brand Commitments

- Nome: Nika. Voz: direta, honesta, sem venda; avisos que custam dizer são ditos.
- Sistema visual existente ("bilhete técnico", `src/styles.css`): preservar.
- Português do Brasil em toda a UI.

## Evidence on Hand

- Logs reais do bloqueio e da solução (`docs/discord-tunnel.md` §1, memória do projeto).
- Scripts de servidor validados até a rede (VM pendente de capacidade na Oracle em 2026-10-20).
- Nenhum depoimento, número de usuários ou benchmark: não inventar.

## Product Principles

1. Só o Discord passa pelo túnel; o resto do PC é invisível ao Nika.
2. A verdade vem do sistema (disco, driver, handshake), nunca do que o app "acha" que fez.
3. Quem é leigo não vê terminal, chave ou IP; quem é dono vê tudo e decide no terminal.
4. Falhar aberto e dizer o que falhou, com a ação que resolve.
5. Honestidade acima de conforto: custos (admin, driver, IP do dono) ficam visíveis.
