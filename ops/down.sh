#!/usr/bin/env bash
# Tears the cluster down between sessions: terraform destroy in infra/cluster
# only. The bootstrap root (ECR repository, GitHub role, budget) stays.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
local_dir="$HOME/.miniarcade"
tfvars="$local_dir/cluster.tfvars"
export AWS_PROFILE="${AWS_PROFILE:-miniarcade}"

if ! aws sts get-caller-identity >/dev/null 2>&1; then
  echo "Not logged in to AWS. Run: aws sso login --profile $AWS_PROFILE" >&2
  exit 1
fi
if [[ ! -f "$tfvars" ]]; then
  echo "No $tfvars: ops/up.sh has never run, so there is nothing to destroy." >&2
  exit 0
fi

tf=(terraform -chdir="$repo/infra/cluster")
"${tf[@]}" init -input=false >/dev/null
# my_ip only shapes security group rules, which are being deleted anyway.
"${tf[@]}" destroy -input=false -auto-approve -var-file="$tfvars" -var "my_ip=127.0.0.1"

left="$("${tf[@]}" state list)"
if [[ -n "$left" ]]; then
  echo "These are still in the cluster state:" >&2
  echo "$left" >&2
  exit 1
fi

# The node is gone, and its host key, inventory and kubeconfig with it.
rm -f "$local_dir/known_hosts" "$local_dir/inventory.ini" "$local_dir/kubeconfig"
echo "Cluster destroyed; its Terraform state is empty. Bootstrap (ECR, GitHub role, budget) remains."
