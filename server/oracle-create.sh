#!/usr/bin/env bash
# Cria, na Oracle Cloud, a saída WireGuard do Nika: rede, porta liberada e VM.
#
# Roda no Cloud Shell do console da Oracle, que já vem autenticado e com o
# `oci` instalado. Não instala o WireGuard: isso é o `install.sh`, executado
# depois por SSH. O script pode ser rodado de novo sem estrago: cada recurso é
# procurado pelo nome antes de ser criado.
#
#   bash oracle-create.sh "ssh-ed25519 AAAA... comentario"
#   NIKA_SSH_KEY="ssh-ed25519 AAAA..." bash oracle-create.sh
#
# Tudo fica na região home da conta, que é onde o plano Always Free vale. Uma
# região brasileira é recusada: a saída precisa estar fora do Brasil.
set -euo pipefail

ssh_key="${1:-${NIKA_SSH_KEY:-}}"
if [ -z "$ssh_key" ]; then
  echo "faltou a chave SSH pública (argumento 1 ou NIKA_SSH_KEY)" >&2
  exit 2
fi

tenancy="${OCI_TENANCY:-}"
if [ -z "$tenancy" ]; then
  echo "OCI_TENANCY vazio: rode no Cloud Shell do console da Oracle" >&2
  exit 2
fi

region="$(oci iam region-subscription list --tenancy-id "$tenancy" \
  --query 'data[?"is-home-region"] | [0]."region-name"' --raw-output)"
case "$region" in
  sa-saopaulo-*|sa-vinhedo-*)
    echo "a região home da conta é $region (Brasil): o IP de saída seria brasileiro" >&2
    echo "e o Go Live continuaria bloqueado. Crie a conta com outra região home." >&2
    exit 1
    ;;
esac
echo "região home: $region"

# Toda chamada vai para a região home, mesmo que o console esteja em outra.
o() { oci --region "$region" "$@"; }

# Procura por nome; imprime o OCID ou nada.
find_id() { o "$@" --query 'data[0].id' --raw-output 2>/dev/null || true; }

compartment="$tenancy"
ad="$(o iam availability-domain list --compartment-id "$compartment" \
  --query 'data[0].name' --raw-output)"

# ---------------------------------------------------------------- rede ------
vcn="$(find_id network vcn list --compartment-id "$compartment" --display-name nika-vcn)"
if [ -z "$vcn" ]; then
  echo "criando VCN..."
  vcn="$(o network vcn create --compartment-id "$compartment" \
    --cidr-blocks '["10.0.0.0/16"]' --display-name nika-vcn --dns-label nikavcn \
    --wait-for-state AVAILABLE --query 'data.id' --raw-output)"
fi

igw="$(find_id network internet-gateway list --compartment-id "$compartment" --vcn-id "$vcn")"
if [ -z "$igw" ]; then
  echo "criando internet gateway..."
  igw="$(o network internet-gateway create --compartment-id "$compartment" \
    --vcn-id "$vcn" --is-enabled true --display-name nika-igw \
    --wait-for-state AVAILABLE --query 'data.id' --raw-output)"
fi

route_table="$(o network vcn get --vcn-id "$vcn" \
  --query 'data."default-route-table-id"' --raw-output)"
o network route-table update --rt-id "$route_table" --force \
  --route-rules "[{\"destination\":\"0.0.0.0/0\",\"destinationType\":\"CIDR_BLOCK\",\"networkEntityId\":\"$igw\"}]" \
  >/dev/null

# SSH para administrar, UDP 51820 para o WireGuard e ICMP "fragmentation
# needed", sem o qual o path MTU discovery quebra em silêncio.
ingress='[
  {"source":"0.0.0.0/0","protocol":"6","isStateless":false,
   "tcpOptions":{"destinationPortRange":{"min":22,"max":22}}},
  {"source":"0.0.0.0/0","protocol":"17","isStateless":false,
   "udpOptions":{"destinationPortRange":{"min":51820,"max":51820}}},
  {"source":"0.0.0.0/0","protocol":"1","isStateless":false,
   "icmpOptions":{"type":3,"code":4}}
]'
egress='[{"destination":"0.0.0.0/0","protocol":"all","isStateless":false}]'

security_list="$(find_id network security-list list --compartment-id "$compartment" \
  --vcn-id "$vcn" --display-name nika-sl)"
if [ -z "$security_list" ]; then
  echo "criando security list (SSH + UDP 51820)..."
  security_list="$(o network security-list create --compartment-id "$compartment" \
    --vcn-id "$vcn" --display-name nika-sl \
    --ingress-security-rules "$ingress" --egress-security-rules "$egress" \
    --wait-for-state AVAILABLE --query 'data.id' --raw-output)"
else
  o network security-list update --security-list-id "$security_list" --force \
    --ingress-security-rules "$ingress" --egress-security-rules "$egress" >/dev/null
fi

subnet="$(find_id network subnet list --compartment-id "$compartment" \
  --vcn-id "$vcn" --display-name nika-subnet)"
if [ -z "$subnet" ]; then
  echo "criando subnet pública..."
  subnet="$(o network subnet create --compartment-id "$compartment" --vcn-id "$vcn" \
    --cidr-block 10.0.0.0/24 --display-name nika-subnet --dns-label nika \
    --route-table-id "$route_table" --security-list-ids "[\"$security_list\"]" \
    --prohibit-public-ip-on-vnic false \
    --wait-for-state AVAILABLE --query 'data.id' --raw-output)"
fi

# ------------------------------------------------------------------ VM ------
instance="$(find_id compute instance list --compartment-id "$compartment" \
  --display-name nika-exit --lifecycle-state RUNNING)"

key_file="$(mktemp)"
trap 'rm -f "$key_file"' EXIT
printf '%s\n' "$ssh_key" > "$key_file"

ubuntu_image() {
  o compute image list --compartment-id "$compartment" \
    --operating-system "Canonical Ubuntu" --operating-system-version "24.04" \
    --shape "$1" --sort-by TIMECREATED --sort-order DESC \
    --query 'data[0].id' --raw-output
}

launch() {
  local shape="$1"; shift
  o compute instance launch --compartment-id "$compartment" \
    --availability-domain "$ad" --shape "$shape" "$@" \
    --image-id "$(ubuntu_image "$shape")" --subnet-id "$subnet" \
    --assign-public-ip true --display-name nika-exit \
    --ssh-authorized-keys-file "$key_file" \
    --wait-for-state RUNNING --query 'data.id' --raw-output
}

if [ -z "$instance" ]; then
  # Ampere é a mais folgada do plano grátis, mas vive sem capacidade em várias
  # regiões; a AMD micro quase sempre existe e dá conta de um WireGuard.
  echo "criando VM (Ampere A1, 1 OCPU / 6 GB)..."
  if ! instance="$(launch VM.Standard.A1.Flex --shape-config '{"ocpus":1,"memoryInGBs":6}')"; then
    echo "sem capacidade para Ampere; tentando VM.Standard.E2.1.Micro..."
    instance="$(launch VM.Standard.E2.1.Micro)"
  fi
fi

public_ip="$(o compute instance list-vnics --instance-id "$instance" \
  --query 'data[0]."public-ip"' --raw-output)"

echo
echo "pronto: VM nika-exit em $region"
echo "IP público: $public_ip"
