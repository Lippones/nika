# Recebi um convite. E agora?

Alguém que tem um servidor do Nika te mandou uma linha parecida com esta:

```
nika://convite/IyBOaWthLVNlcnZlcjogRmlsaXBlIMK3IFNhbnRpYWdvCiMgTmlrYS1QZWVy…
```

Isso é o seu passe para usar o servidor dessa pessoa. Com ele, **o Discord do
seu PC passa a sair pela internet pelo servidor dela**, e o Go Live, a voz e o
vídeo voltam a funcionar. O resto do seu computador não muda nada.

## O que fazer

1. Instale o Nika (o `.exe` que veio junto, ou o da página de Releases).
2. Na primeira tela, clique em **Tenho um convite**.
3. Cole a linha inteira, do `nika://` até o fim. O Nika mostra na hora para
   quem é o convite e de quem é o servidor. Se aparecer "convite não
   reconhecido", copie de novo: faltou um pedaço.
4. Clique em **Usar convite e reabrir como admin**. O Windows pergunta se o Nika
   pode fazer alterações (UAC): é o driver que separa o tráfego do Discord do
   resto do PC. Diga sim.
5. O Nika reabre e a faixa **Saída do Discord** mostra "no ar" e "Handshake:
   agora". Abra o Discord. Pronto.

Se já pulou a primeira tela, a mesma coisa está na faixa **Saída do Discord**
da janela principal.

## O que você precisa saber

- **O convite é seu.** Ele tem uma chave única. Se repassar, a outra pessoa
  entra como você, e quem te convidou pode cortar o acesso dos dois.
- **Não é anonimato.** O Discord passa a ver o IP do servidor da pessoa que te
  convidou, não o seu. Para ela, e para a Oracle (onde o servidor roda), o
  tráfego é dela. Use com quem confia em você, e só no Nika.
- **Só o Discord passa pelo servidor.** Navegador, jogos, Steam, tudo o mais
  continua saindo direto do seu PC. É de propósito: o servidor é pequeno e
  grátis.
- **Precisa de administrador** toda vez que o Nika é aberto sem ser como admin.
  A faixa mostra o botão **Reabrir como administrador** quando for o caso. Com
  "Ligar ao abrir o Nika" marcado e o Nika aberto como admin, liga sozinho.

## Se algo der errado

| Sintoma | O que é | O que fazer |
|---|---|---|
| "Sem resposta do servidor" | O servidor está desligado, ou o seu convite foi revogado | Fale com quem te convidou |
| "Componente ausente" | O instalador que você usou veio sem o driver | Peça o instalador completo a quem te convidou |
| Discord abre mas Go Live fica "carregando" | O túnel não está ligado ou o Discord abriu antes dele | Confira "no ar" na faixa e reinicie o Discord |
| Voz com atraso | Normal: a voz dá uma volta pelo servidor (Chile, por exemplo) | Nada; é o custo de sair por fora |

Para desligar, **Desligar** na faixa. Para apagar o convite do PC, **Esquecer
convite** (com a saída desligada).

## Quero o meu próprio servidor

Dá, e é grátis: [oracle-exit.md](oracle-exit.md) é o passo a passo. Aí você é
quem convida.
