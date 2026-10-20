#!/usr/bin/env bash
# Instala a saída WireGuard do Nika numa VM Ubuntu (feito para a Oracle Cloud).
#
#   scp server/install.sh server/nika-peer ubuntu@IP:/tmp/
#   ssh ubuntu@IP sudo bash /tmp/install.sh "Filipe · Santiago"
#
# O argumento é o rótulo do servidor: é o que cada convidado vê no Nika como
# "servidor de quem". Sem argumento, fica o hostname; rodar de novo com um
# rótulo troca só o rótulo.
#
# Idempotente: rodar de novo não troca a chave do servidor nem perde perfis.
#
# Decisões que não são óbvias:
# - MTU 1380 fixo. A VNIC da Oracle tem MTU 9000 e o wg-quick deduziria 8920,
#   o que gera pacotes que não passam pela internet (MTU 1500, ou 1492 nas
#   conexões PPPoE comuns no Brasil). 1380 cabe nas duas com folga.
# - IPv6 entra no túnel só para ser recusado na hora (tcp-reset/ICMP). Sem
#   isso, um PC com IPv6 abriria o gateway do Discord por IPv6 fora do túnel e
#   a trava voltaria; com a recusa imediata, o Chromium cai para IPv4 (que sai
#   pela VM) sem esperar timeout. A VM não precisa ter IPv6 público.
# - A imagem Ubuntu da Oracle traz um iptables com REJECT no fim de INPUT e
#   FORWARD; as regras daqui entram antes dele. Nada de ufw: ele briga com essas
#   regras e pode trancar o SSH.
set -euo pipefail

[ "$(id -u)" -eq 0 ] || exec sudo bash "$0" "$@"

here="$(cd "$(dirname "$0")" && pwd)"
label="${1:-}"
port=51820
net4=10.66.0
net6=fd4e:696b:61
mtu=1380

export DEBIAN_FRONTEND=noninteractive
apt-get update -y
apt-get install -y wireguard-tools iptables iptables-persistent curl

install -d -m 700 /etc/wireguard /etc/nika /etc/nika/peers

if [ -n "$label" ]; then
  printf '%s\n' "$label" > /etc/nika/label
elif [ ! -s /etc/nika/label ]; then
  hostname > /etc/nika/label
fi

if [ ! -s /etc/wireguard/server.key ]; then
  (umask 077 && wg genkey > /etc/wireguard/server.key)
fi
wg pubkey < /etc/wireguard/server.key > /etc/wireguard/server.pub

egress="$(ip -4 route show default | awk '{print $5; exit}')"
if [ -z "$egress" ]; then
  echo "sem rota padrão IPv4: não sei por qual interface sair" >&2
  exit 1
fi

# O IP público é o que o Discord vai ver. Na Oracle o NAT é 1:1, então o IP de
# saída da VM é o mesmo do endpoint.
if [ ! -s /etc/nika/endpoint ]; then
  public_ip="$(curl -fsS --max-time 10 https://1.1.1.1/cdn-cgi/trace | sed -n 's/^ip=//p')"
  [ -n "$public_ip" ] || { echo "não consegui descobrir o IP público" >&2; exit 1; }
  echo "$public_ip:$port" > /etc/nika/endpoint
fi

# Só o cabeçalho é gerado aqui; os [Peer] são do nika-peer e sobrevivem a uma
# reinstalação porque o arquivo existente é preservado.
if [ ! -s /etc/wireguard/wg0.conf ]; then
  (umask 077 && cat > /etc/wireguard/wg0.conf <<EOF
[Interface]
Address = $net4.1/24, $net6::1/64
ListenPort = $port
MTU = $mtu
PostUp = wg set %i private-key /etc/wireguard/server.key
EOF
)
fi

cat > /etc/sysctl.d/99-nika.conf <<EOF
net.ipv4.ip_forward = 1
net.ipv6.conf.all.forwarding = 1
EOF
sysctl --system >/dev/null

# Insere a regra no topo da chain só se ela ainda não existe.
ensure() {
  local cmd="$1" table="$2" chain="$3"; shift 3
  "$cmd" -t "$table" -C "$chain" "$@" 2>/dev/null || "$cmd" -t "$table" -I "$chain" 1 "$@"
}

ensure iptables filter INPUT -p udp --dport "$port" -j ACCEPT
ensure iptables filter FORWARD -i "$egress" -o wg0 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
ensure iptables filter FORWARD -i wg0 -o "$egress" -j ACCEPT
ensure iptables nat POSTROUTING -s "$net4.0/24" -o "$egress" -j MASQUERADE
ensure iptables mangle FORWARD -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu

# Ordem importa: a recusa genérica entra primeiro e a de TCP por cima dela.
ensure ip6tables filter FORWARD -i wg0 -j REJECT --reject-with icmp6-adm-prohibited
ensure ip6tables filter FORWARD -i wg0 -p tcp -j REJECT --reject-with tcp-reset

netfilter-persistent save >/dev/null

install -m 755 "$here/nika-peer" /usr/local/sbin/nika-peer

systemctl enable wg-quick@wg0 >/dev/null 2>&1
systemctl restart wg-quick@wg0

echo "saída \"$(cat /etc/nika/label)\" pronta em $(cat /etc/nika/endpoint), chave pública $(cat /etc/wireguard/server.pub)"
echo "crie um convite com: sudo nika-peer add <nome>"
