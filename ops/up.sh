#!/usr/bin/env bash
# Creates the cluster, or brings it up to date: the EC2 node with Terraform
# (infra/cluster), then k3s with Ansible (infra/ansible). Safe to re-run, for
# example after your public IP changes.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
local_dir="$HOME/.miniarcade"
key="$local_dir/node_ed25519"
tfvars="$local_dir/cluster.tfvars"
inventory="$local_dir/inventory.ini"
export AWS_PROFILE="${AWS_PROFILE:-miniarcade}"

if ! aws sts get-caller-identity >/dev/null 2>&1; then
  echo "Not logged in to AWS. Run: aws sso login --profile $AWS_PROFILE" >&2
  exit 1
fi

# Everything written below is local-only and readable by you alone.
umask 077
mkdir -p "$local_dir"
# First run only: the node's SSH key and this root's tfvars file.
[[ -f "$key" ]] || ssh-keygen -q -t ed25519 -N '' -C miniarcade-node -f "$key"
[[ -f "$tfvars" ]] || printf 'public_key_path = "%s"\n' "$key.pub" >"$tfvars"

my_ip="$(curl -fsS --max-time 10 https://checkip.amazonaws.com | tr -d '[:space:]')"
if [[ ! "$my_ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then
  echo "Couldn't read your public IP from checkip.amazonaws.com (got '$my_ip')." >&2
  exit 1
fi
echo "Your public IP: $my_ip (only it may reach SSH and the Kubernetes API)"

tf=(terraform -chdir="$repo/infra/cluster")
"${tf[@]}" init -input=false >/dev/null
"${tf[@]}" apply -input=false -auto-approve -var-file="$tfvars" -var "my_ip=$my_ip"
node_ip="$("${tf[@]}" output -raw node_ip)"

cat >"$inventory" <<EOF
[node]
$node_ip ansible_user=ubuntu ansible_ssh_private_key_file=$key node_ip=$node_ip
EOF

ANSIBLE_CONFIG="$repo/infra/ansible/ansible.cfg" \
  ansible-playbook -i "$inventory" "$repo/infra/ansible/site.yml"

export KUBECONFIG="$local_dir/kubeconfig"
echo
echo "Site:     http://$node_ip/"
echo "kubectl:  export KUBECONFIG=$KUBECONFIG"
# ArgoCD and Grafana aren't exposed outside the cluster; each line appears once
# that part is installed.
if kubectl get namespace argocd >/dev/null 2>&1; then
  echo "ArgoCD:   kubectl -n argocd port-forward svc/argocd-server 8081:80  (then http://localhost:8081)"
fi
if kubectl get namespace monitoring >/dev/null 2>&1; then
  echo "Grafana:  kubectl -n monitoring port-forward svc/kube-prometheus-stack-grafana 3000:80  (then http://localhost:3000)"
fi
