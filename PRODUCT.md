# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Dono da saída** (o Filipe, e quem mais seguir `docs/oracle-exit.md`): sobe uma VM na Oracle Cloud, gerencia quem tem acesso pelo terminal (`nika-peer add|remove|list` via SSH) e usa o Nika no próprio PC para o Go Live.
- **Convidados**: 3 a 10 amigos leigos, no Windows, que só querem que o Go Live e a voz do Discord voltem a funcionar. Recebem um convite do dono por mensagem (Discord/WhatsApp), colam no Nika e seguem. Nenhum deles usa terminal.

## Product Purpose

Nika é um app de bandeja do Windows com um único trabalho: fazer o Discord sair por outro IP. Desde o bloqueio de Go Live e vídeo no Brasil (agosto de 2026), isso significa um túnel WireGuard até uma VM do dono, pelo qual passam TCP e UDP dos processos do Discord, e nada mais do PC. Sucesso: o Go Live anda (`receiver count: 1`) para o dono e para cada convidado, sem ninguém configurar rota, DNS ou firewall. (A primeira versão era um proxy Tor; foi removida em outubro de 2026 porque não alcançava a mídia.)

## Positioning

Só o tráfego do Discord sai pela VM, por processo, sem adaptador de rede nem VPN de sistema. Uma VPN comum roteia o PC inteiro; o proxy do Discord sozinho não leva UDP. O Nika leva as duas coisas (TCP e UDP) e nada além delas.

## Operating Context

- Convite = o `.conf` WireGuard que `nika-peer add <nome>` imprime na VM. Um por pessoa. É o único passe: a VM ignora quem não tem chave. Revogação imediata com `nika-peer remove`.
- Entrega do convite: texto copiado numa DM. Não há canal dentro do app para isso.
- Ligar o túnel exige administrador (driver WinDivert). O Nika reabre pedindo UAC.
- A VM é grátis (Oracle Always Free) e pequena: poucos usuários, prioridade para o Go Live.
- Tudo que sai da VM sai no IP e na conta do dono: convite só para gente de confiança, e só para uso no Nika.

## Capabilities and Constraints

- Túnel do Discord: importar perfil, ligar/desligar, estado (handshake, tráfego, fluxos), esquecer perfil, reabrir elevado, ligar ao abrir. Spec em `docs/discord-tunnel.md`.
- Gestão de convidados fica fora do app, no terminal (decisão de produto: menos segredo e menos superfície no app).
- Não é anonimato. A UI diz isso sempre.
- Repositório público; o código não pode presumir uma VM específica.

## Brand Commitments

- Nome: Nika. Voz: direta, honesta, sem venda; avisos que custam dizer são ditos.
- Sistema visual existente ("bilhete técnico", `src/styles.css`): preservar.
- Português do Brasil em toda a UI.

## Evidence on Hand

- Logs reais do bloqueio e da solução (`docs/discord-tunnel.md` §1).
- Scripts de servidor validados até a rede (VM pendente de capacidade na Oracle em 2026-10-20).
- Nenhum depoimento, número de usuários ou benchmark: não inventar.

## Product Principles

1. Só o Discord passa pelo túnel; o resto do PC é invisível ao Nika.
2. A verdade vem do sistema (disco, driver, handshake), nunca do que o app "acha" que fez.
3. Quem é leigo não vê terminal, chave ou IP; quem é dono vê tudo e decide no terminal.
4. Falhar aberto e dizer o que falhou, com a ação que resolve.
5. Honestidade acima de conforto: custos (admin, driver, IP do dono) ficam visíveis.
