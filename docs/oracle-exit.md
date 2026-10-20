# Saída própria na Oracle Cloud — passo a passo do dono

Isto cria a VM que o túnel do Nika usa como saída do Discord
([docs/discord-tunnel.md](discord-tunnel.md)) e explica como convidar amigos.
Tudo cabe no plano **Always Free** da Oracle: uma VM pequena, 10 TB/mês de
saída, sem cobrança. Precisa de uma conta na Oracle Cloud e de paciência com a
capacidade (ver §4).

Quem **recebe** um convite não precisa de nada disto: [convite.md](convite.md).

> **A região home da conta não pode ser no Brasil.** O plano grátis vale só na
> região home, e uma saída em São Paulo ou Vinhedo teria IP brasileiro, então o
> Go Live continuaria bloqueado. Ao criar a conta, escolha uma região fora do
> Brasil. **Chile Central (Santiago)** é a mais perto. Depois de criada, a home
> não muda. O `oracle-create.sh` recusa continuar se a home for brasileira.

## 1. Chave SSH

No WSL ou PowerShell, se ainda não tiver uma só para isto:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/nika-oracle -C nika-oracle
```

## 2. OCI CLI no seu PC

Dois caminhos. O Cloud Shell do console da Oracle já vem com o `oci` e
autenticado, mas a sessão expira e o laço de tentativas (§4) precisa rodar por
horas. Por isso o recomendado é o CLI local.

**Instalar sem root** (o instalador oficial da Oracle exige `python3-venv`; com
o `uv` não precisa):

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh && export PATH="$HOME/.local/bin:$PATH" && uv tool install --python 3.12 oci-cli
```

**Autenticar pelo navegador**, sem chave de API nem OCID digitado:

```bash
oci session authenticate --region sa-santiago-1 --profile-name nika
```

Ele abre (ou imprime) um link; faça login na mesma aba. A aba redireciona para
`localhost:8181` e o terminal diz "Completed browser authentication process".
No WSL, se nada abrir, cole o link no navegador do Windows: o `localhost` do
WSL é alcançável de lá. Se o login aconteceu em outra aba e o redirecionamento
não veio, abra o link de novo: já logado, ele completa na hora.

O token vale 1 hora e renova por até 24 h (`oci session refresh --profile nika`).
Depois que a VM existir, o Nika não fala mais com a API da Oracle.

> **Chave de API em vez de sessão.** `oci setup config` também funciona, mas a
> chave privada que ele gera vem com senha (precisa de `pass_phrase=` no
> config) e a chave pública tem que ser cadastrada em *Meu perfil → Chaves de
> API*. Mais passos para errar; a sessão pelo navegador evita todos.

## 3. Criar rede e VM

```bash
export OCI_CLI_PROFILE=nika OCI_CLI_AUTH=security_token
export OCI_TENANCY=$(awk -F= '/^\[nika\]/{f=1} f&&/^tenancy=/{print $2; exit}' ~/.oci/config)
bash server/oracle-create.sh "$(cat ~/.ssh/nika-oracle.pub)"
```

O script é idempotente: pode rodar de novo que ele acha o que já existe. Ele
cria VCN, internet gateway, security list (SSH + UDP 51820 + ICMP de MTU),
subnet e a VM `nika-exit`. Tenta primeiro a Ampere A1 (1 OCPU / 6 GB); se a
região estiver sem capacidade, cai para a AMD `E2.1.Micro`, que dá conta de um
WireGuard. No fim imprime o **IP público**.

## 4. "Out of host capacity"

É a resposta normal do plano grátis: a capacidade aparece e some ao longo do
dia, e Santiago tem um único domínio de disponibilidade. Não é erro seu.

Deixe um laço insistindo. Ele roda o `oracle-create.sh` a cada 5 minutos,
renova o token sozinho e para quando conseguir:

```bash
server/oracle-retry.sh ~/.ssh/nika-oracle.pub /tmp/nika-retry.log
```

Acompanhe com `tail -f /tmp/nika-retry.log`. Pode levar de minutos a dias.
O que ajuda:

- **Madrugada e início da manhã** costumam ter vaga.
- **Upgrade para Pay As You Go** dá prioridade de capacidade; os recursos
  Always Free continuam sem custo. Se for por aí, crie junto uma *quota policy*
  "zero" para shapes pagos (Governance → Quota Policies), para nada fora do
  grátis poder ser criado nem por engano. Saída de rede acima de 10 TB/mês
  custaria uns US$ 8 por TB extra; com Discord isso não acontece.
- Numa conta que ficou só no Free Tier, não há como ser cobrado: o que passa do
  limite é recusado ou parado. Em troca, a Oracle pode reciclar VMs grátis
  ociosas por 7 dias (o keepalive do Nika conta como uso, mas não é garantia).

## 5. Instalar o WireGuard na VM

Do seu PC, com o IP que o script imprimiu e um rótulo para os seus convidados
verem ("Filipe · Santiago", por exemplo):

```bash
scp -i ~/.ssh/nika-oracle server/install.sh server/nika-peer ubuntu@IP:/tmp/
```

```bash
ssh -i ~/.ssh/nika-oracle ubuntu@IP sudo bash /tmp/install.sh "Filipe · Santiago"
```

O `install.sh` instala `wireguard-tools`, gera a chave do servidor, fixa MTU
1380, liga o encaminhamento, escreve as regras de iptables **antes** do REJECT
que a imagem da Oracle traz, e sobe o `wg0`. Também é idempotente. Rodar de novo
com outro rótulo troca só o rótulo.

## 6. Criar convites

Um por pessoa, inclusive você:

```bash
ssh -i ~/.ssh/nika-oracle ubuntu@IP sudo nika-peer add filipe
```

A saída tem duas partes: o perfil WireGuard legível e, embaixo, o **convite de
uma linha** (`nika://convite/…`). É essa linha que você manda por mensagem; ela
não quebra e o Nika aceita colada. Para você mesmo, cole no Nika do seu PC
(faixa **Saída do Discord** → **Usar convite**).

Gestão, sempre no terminal:

| Comando | Faz |
|---|---|
| `nika-peer add <nome>` | cria o convite (ou mostra o que já existe) |
| `nika-peer show <nome>` | imprime o convite de novo |
| `nika-peer list` | nomes, IPs no túnel e último handshake de cada um |
| `nika-peer remove <nome>` | revoga na hora; o Nika da pessoa passa a dizer "sem resposta do servidor" |

Nomes: minúsculas, números e hífen, até 32 caracteres.

**Quem convidar.** Tudo que sai da VM sai no seu IP e na sua conta. Com o Nika,
só o Discord da pessoa passa por você, e o Discord já cifra tudo; você vê
destinos e volume, não conteúdo. Mas o mesmo perfil funciona no app oficial do
WireGuard como VPN completa, e aí o tráfego inteiro dela é atribuído a você.
Convite é para gente de confiança, e só para usar no Nika. A VM é pequena:
poucas pessoas, prioridade para o Go Live.

## 7. Validar antes do Nika (recomendado)

O perfil funciona no [app oficial do WireGuard](https://www.wireguard.com/install/)
como VPN completa. Importe lá, ative, e confira:

1. `https://1.1.1.1/cdn-cgi/trace` mostra `ip=` igual ao IP da VM e `loc=CL`.
2. Abra o Discord e inicie um Go Live. Em
   `%APPDATA%\discord\logs\discord-webrtc_0` deve aparecer `receiver count: 1`
   quando alguém assistir.

Se funciona, o servidor está certo. Desative a VPN completa e use o Nika, que
faz o mesmo só para o Discord.

## 8. Ligar no Nika

1. Na faixa **Saída do Discord**, cole o seu convite → **Usar convite**.
2. **Reabrir como administrador** (uma vez por sessão do Nika; o driver exige).
3. O Nika reabre e liga. "Handshake: agora" significa que a VM respondeu.
4. Abra o Discord. "Fluxos" passa a contar as portas do Discord em uso.

## Manutenção

| Situação | O que fazer |
|---|---|
| "Sem resposta do servidor" para todo mundo | `ssh ubuntu@IP sudo wg show`; se o `wg0` não existe, `sudo systemctl restart wg-quick@wg0` |
| Só uma pessoa sem resposta | `nika-peer list`: ela foi removida, ou o convite dela está incompleto |
| A Oracle avisou que vai recuperar a VM por ociosidade | Ver §4: PAYG resolve; ou aceite recriar com o `oracle-create.sh` + `install.sh` (as chaves dos convidados se perdem) |
| Troquei de PC | `nika-peer show filipe` imprime o convite de novo |
| Quero apagar tudo | Console da Oracle: Compute → Instances → `nika-exit` → Terminate; Networking → VCN `nika-vcn` → Delete |

## O que a VM vê

Só os pacotes do Discord de quem tem convite: gateway, API, voz, vídeo. O IP
que o Discord enxerga é o IP público da VM (NAT 1:1 da Oracle). Não é
anonimato: a VM está na sua conta, e você sabe quem convidou.
