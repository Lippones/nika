#!/usr/bin/env bash
# Insiste no oracle-create.sh até a Oracle ter capacidade ("Out of host
# capacity" é a resposta normal do plano grátis em várias regiões; a vaga
# aparece e some ao longo do dia). Renova o token de sessão do OCI CLI antes
# de ele expirar (vale 1 h, renovável por até 24 h a partir do login).
#
# Roda no seu PC (não no Cloud Shell), depois de `oci session authenticate
# --profile-name nika`. Ver docs/oracle-exit.md.
#
#   server/oracle-retry.sh ~/.ssh/nika-oracle.pub /tmp/nika-retry.log [tentativas]
set -u
export PATH="$HOME/.local/bin:$PATH" SUPPRESS_LABEL_WARNING=True PYTHONWARNINGS=ignore
export OCI_CLI_PROFILE="${OCI_CLI_PROFILE:-nika}" OCI_CLI_AUTH="${OCI_CLI_AUTH:-security_token}"
export OCI_TENANCY="${OCI_TENANCY:-$(awk -F= -v p="[$OCI_CLI_PROFILE]" '$0==p{f=1} f&&/^tenancy=/{print $2; exit}' ~/.oci/config)}"
cd "$(dirname "$0")/.."
key="$(cat "$1")"
log="$2"; interval=300; max_tries=${3:-200}
last_refresh=$(date +%s)
for i in $(seq 1 "$max_tries"); do
  now=$(date +%s)
  if [ $((now - last_refresh)) -gt 2400 ]; then
    oci session refresh --profile "$OCI_CLI_PROFILE" >/dev/null 2>&1 && last_refresh=$now || echo "$(date '+%H:%M') refresh do token falhou" >> "$log"
  fi
  if out="$(bash server/oracle-create.sh "$key" 2>&1)"; then
    echo "$out" | grep -v "^Warning: To increase" >> "$log"
    echo "SUCESSO na tentativa $i em $(date '+%d/%m %H:%M')" >> "$log"
    exit 0
  fi
  msg=$(echo "$out" | grep -oE '"message": "[^"]+"' | tail -1)
  echo "$(date '+%H:%M') tentativa $i falhou: ${msg:-erro desconhecido}" >> "$log"
  sleep "$interval"
done
echo "DESISTI depois de $max_tries tentativas" >> "$log"
exit 1
