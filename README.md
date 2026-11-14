# MiniArcade

A small DevOps learning lab: a two-player XO game in the browser, taken
through the whole DevOps cycle on AWS.

| Stage | Here |
|---|---|
| Plan | One GitHub Issue per build phase and per backlog game, on the MiniArcade project board |
| Code | One Rust service (axum, Tokio, Redis) that also serves a plain HTML/JS page; test-driven |
| Build | One multi-stage Dockerfile; a distroless, non-root image tagged with the short commit SHA |
| Test | On every PR: fmt, clippy, unit and Redis integration tests with a 70% coverage floor, Trivy, infra lint |
| Release | Merging an app change pushes the image to private ECR through GitHub OIDC; you approve the deploy |
| Deploy | Terraform (bootstrap + cluster), Ansible (k3s, Argo CD), Argo CD syncs `deploy/` (GitOps) |
| Monitor | Prometheus + Grafana dashboard, Loki logs, three email alerts, an AWS Budget alert |
| Operate | `ops/up.sh` / `ops/down.sh` between sessions, a k6 load test, rollback by reverting a deploy commit |

```
 you ── PR ──▶ GitHub ── CI: rust · security · infra ──▶ main
                                                          │ app change merged
                                                          ▼
                Release: publish ── OIDC ──▶ ECR  miniarcade:<short-sha>
                         deploy  ── your approval ──▶ commit "deploy: <short-sha>" to deploy/
                                                          │ Argo CD syncs
                                                          ▼
 player ── HTTP :80 ──▶ EC2 node (k3s) ── Traefik ──▶ app ×2 ──▶ Redis
                         └── Prometheus · Alertmanager · Grafana · Loki · Alloy ──▶ email alerts
```

## Repository layout

| Path | What |
|---|---|
| `app/` | The Rust service, its tests, the page (`app/static/`) and the Dockerfile |
| `.github/workflows/` | `ci.yml` (every PR), `release.yml` (publish and deploy) |
| `infra/bootstrap/` | Terraform, applied once: ECR, the GitHub OIDC role, the $20 budget |
| `infra/cluster/` | Terraform, created and destroyed each session: VPC, security group, one m7i-flex.large |
| `infra/ansible/` | k3s, the ECR credential provider, Argo CD, the monitoring Secrets |
| `deploy/` | Everything Argo CD syncs: the app, Redis, Ingress, monitoring Applications, alerts, dashboard |
| `ops/` | `up.sh`, `down.sh`, `loadtest.sh` |

Local-only files live in `~/.miniarcade/` (folder `700`, files `600`) and never
in Git: one tfvars file per Terraform root, `secrets.yml`, the kubeconfig, the
SSH key, the inventory and the generated Grafana password. The Terraform state
stays in each root folder, gitignored.

## Setup from zero

### Tools (WSL2 Ubuntu 24.04)

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.100.0 | `rustup` installs it from `app/rust-toolchain.toml` |
| Docker | Desktop with WSL integration | builds the image, runs Redis for the tests |
| Terraform | 1.17.x | both roots require `~> 1.17.0` |
| Ansible | ansible-core 2.21.x, ansible-lint 26.11 | 2.22 needs Python 3.13; Ubuntu 24.04 ships 3.12 |
| kubectl | 1.36 or 1.37 | the cluster runs Kubernetes 1.37 |
| AWS CLI | v2 | |
| GitHub CLI (`gh`) | any recent | |
| k6 | 2.4.0 | for `ops/loadtest.sh` |
| jq, curl, ssh | from Ubuntu | |

### 1. GitHub CLI

```bash
gh auth login --hostname github.com --git-protocol https --web --scopes workflow,project
gh auth setup-git
```

### 2. AWS access (IAM Identity Center)

In the AWS console, as the root user, once:

1. In region ap-southeast-1, open **IAM Identity Center → Enable** (with AWS
   Organizations).
2. **Users → Add user** for yourself; accept the invitation and set up MFA.
3. **Permission sets → Create → Predefined → AdministratorAccess**, with an
   8-hour session.
4. **AWS accounts →** your account **→ Assign users →** your user **→
   AdministratorAccess**.
5. Optional: **Account → IAM user and role access to Billing information →
   Activate**, so the console shows Billing and Budgets to that user.

Then on your machine, with the access portal URL from the Identity Center
dashboard:

```bash
aws configure sso --profile miniarcade      # region ap-southeast-1, output json
aws sso login --profile miniarcade --use-device-code --no-browser
aws sts get-caller-identity --profile miniarcade
```

### 3. Local secrets

```bash
mkdir -p -m 700 ~/.miniarcade
(umask 077; echo 'alert_email = "<ALERT_EMAIL>"' > ~/.miniarcade/bootstrap.tfvars)
```

Create an app password for the Gmail account that will send the alerts at
https://myaccount.google.com/apppasswords (it needs 2-Step Verification), and
write `~/.miniarcade/secrets.yml` (`chmod 600`). Paste the password into the
file; never share it anywhere else. Spaces in it are fine.

```yaml
gmail_address: "<sending Gmail address>"
gmail_app_password: "<app password>"
alert_email: "<ALERT_EMAIL>"
```

### 4. Bootstrap (once, by hand)

The GitHub role trusts exactly the OIDC subject GitHub issues for this
repository's `main` branch. Newer repositories use the *immutable* subject,
which carries the owner and repository IDs. Check yours, and if it differs
from the default of `github_sub_prefix` in
`infra/bootstrap/variables.tf`, add it to `bootstrap.tfvars`:

```bash
gh api repos/<owner>/<repo>/actions/oidc/customization/sub
# immutable: "sub_claim_prefix": "repo:<owner>@<owner-id>/<repo>@<repo-id>"
# classic:   use "repo:<owner>/<repo>"
```

```bash
cd infra/bootstrap
export AWS_PROFILE=miniarcade
terraform init
terraform apply -var-file="$HOME/.miniarcade/bootstrap.tfvars"

gh variable set AWS_REGION   --body "$(terraform output -raw aws_region)"
gh variable set AWS_ROLE_ARN --body "$(terraform output -raw aws_role_arn)"
gh variable set ECR_REGISTRY --body "$(terraform output -raw ecr_registry)"
```

This creates the ECR repository (immutable tags, last 10 images kept), the
GitHub OIDC provider and role, and the $20/month budget, which emails
`<ALERT_EMAIL>` at 50% and 100% of actual cost. `ops/down.sh` never touches it.

### 5. GitHub settings

**`production` environment**: you approve every deploy, and only `main` may deploy.

```bash
gh api -X PUT repos/<owner>/<repo>/environments/production --input - <<'EOF'
{ "reviewers": [{ "type": "User", "id": <your numeric user id> }],
  "deployment_branch_policy": { "protected_branches": false, "custom_branch_policies": true } }
EOF
gh api -X POST repos/<owner>/<repo>/environments/production/deployment-branch-policies -f name=main -f type=branch
```

**Deploy key**: the deploy job pushes `deploy: <short-sha>` commits with it.
It must be the repository's only deploy key with write access.

```bash
(umask 077; ssh-keygen -q -t ed25519 -N '' -C "release deploy job" -f /tmp/deploy_key)
gh repo deploy-key add /tmp/deploy_key.pub --allow-write --title "release: deploy job"
gh secret set DEPLOY_KEY --env production < /tmp/deploy_key
shred -u /tmp/deploy_key /tmp/deploy_key.pub
```

**Main-branch ruleset**: a pull request (0 approvals) and the checks `rust`,
`security` and `infra` are required; force pushes and deletion are blocked;
deploy keys are the only bypass. Create it under **Settings → Rules →
Rulesets**, or with `gh api -X POST repos/<owner>/<repo>/rulesets` and a body
containing these rules: `deletion`, `non_fast_forward`, `pull_request` with
`required_approving_review_count: 0`, and `required_status_checks` with the
three contexts (integration ID 15368, GitHub Actions). Set `bypass_actors` to
`[{"actor_id": null, "actor_type": "DeployKey", "bypass_mode": "always"}]`.

### 6. First release and first cluster

```bash
gh workflow run release.yml --ref main
# Actions → the run → Review deployments → production → Approve and deploy
git pull                    # brings in the "deploy: <short-sha>" commit
ops/up.sh
```

## Each session

```bash
aws sso login --profile miniarcade --use-device-code --no-browser
ops/up.sh
```

`ops/up.sh` looks up your public IP (`<MY_IP>`), creates or updates the node
with Terraform, then runs Ansible. A fresh cluster takes about 5 minutes; Argo
CD then needs about 15 more for the first sync of the monitoring stack. It is
safe to re-run, for example when your IP changes: SSH and the Kubernetes API
accept only `<MY_IP>`. It prints:

- **The site**: `http://<NODE_IP>/`
- **kubectl**: `export KUBECONFIG=~/.miniarcade/kubeconfig`
- **Argo CD**: `kubectl -n argocd port-forward svc/argocd-server 8081:80`, then
  http://localhost:8081, user `admin`, with the password command it prints
- **Grafana**: `kubectl -n monitoring port-forward svc/kube-prometheus-stack-grafana 3000:80`,
  then http://localhost:3000, user `admin`, password in `~/.miniarcade/grafana-admin-password`

At the end of the session:

```bash
ops/down.sh
```

It destroys the cluster (`infra/cluster` only), checks that its Terraform
state is empty, and removes the node's kubeconfig, inventory and host key.

**Confirm in Tag Editor that only bootstrap is left**: in the AWS console, open
**Resource Groups & Tag Editor → Tag Editor**, choose regions
**ap-southeast-1** and **us-east-1** (global resources such as IAM appear
there), resource types **All supported resource types**, tag key `Project`
with value `miniarcade`, then **Search resources**. Expect only the bootstrap
resources: the ECR repository `miniarcade` (ap-southeast-1), and the GitHub
OIDC provider and the budget `miniarcade-monthly` (us-east-1). The IAM role
`miniarcade-github-actions` stays too, but Tag Editor doesn't list IAM roles.

For about an hour after `ops/down.sh`, Tag Editor also lists the terminated
instance, its volume and its security-group rules: AWS keeps showing a
terminated instance, tags and all, for a while. To be sure nothing is still
running:

```bash
aws ec2 describe-instances --filters Name=instance-state-name,Values=pending,running,stopping,stopped \
  --query 'length(Reservations[].Instances[])'      # 0
aws ec2 describe-volumes --query 'length(Volumes)'  # 0
```

The same tag search from the CLI:

```bash
aws resourcegroupstaggingapi get-resources --tag-filters Key=Project,Values=miniarcade \
  --query 'ResourceTagMappingList[].ResourceARN'
```

The ECR images cost cents per month; the other bootstrap resources are free.

## Develop locally

The integration tests need Redis on 127.0.0.1:6379 (or set `REDIS_URL`):

```bash
docker run -d --name miniarcade-redis -p 127.0.0.1:6379:6379 redis:8.10.3-alpine
cd app
cargo test
```

Run the server: the app listens on 8080, probes and metrics on 9090. Set
`OPS_ADDR` if something else already uses 9090.

```bash
OPS_ADDR=127.0.0.1:19090 cargo run
```

Open http://localhost:8080, start a game, and open the invite link in a second
tab to play the other side. The page is compiled into the binary, so restart
`cargo run` after editing files in `app/static/`.

Build and run the container image (distroless, runs as UID 65532):

```bash
docker build --tag miniarcade app
docker run --rm -p 8080:8080 -e REDIS_URL=redis://host.docker.internal:6379 miniarcade
```

The app drains for 5 s on SIGTERM, so stop it with `docker stop -t 10`; a
shorter timeout kills it mid-drain.

## Release

1. Merge a pull request that changes `app/`. Every change reaches `main`
   through a PR with the three checks green.
2. **Release** starts by itself. `publish` assumes the AWS role through OIDC,
   builds the image, runs the Trivy gate and pushes
   `<AWS_ACCOUNT_ID>.dkr.ecr.ap-southeast-1.amazonaws.com/miniarcade:<short-sha>`.
   If that tag is already in ECR, which happens on a re-run, it skips straight
   to deploy.
3. `deploy` waits for you: open the run in **Actions → Review deployments →
   production → Approve and deploy**.
4. It commits `deploy: <short-sha>` to `deploy/kustomization.yaml` on `main`.
   That commit triggers no workflow.
5. Argo CD notices within about 3 minutes and rolls the app over, one pod at a
   time, with no downtime. To sync at once:

   ```bash
   kubectl -n argocd annotate application root argocd.argoproj.io/refresh=normal --overwrite
   ```

Re-running an old Release run can't roll the site back: deploy refuses any tag
that isn't newer than the deployed one. To release without an app change, run
`gh workflow run release.yml --ref main`.

## Rollback

Roll back by reverting a deploy commit in a pull request:

```bash
git fetch origin
git log --oneline --grep '^deploy:' origin/main      # pick the deploy commit to undo
git switch -c rollback origin/main
git revert --no-edit <deploy-commit>
git push -u origin rollback
gh pr create --fill
```

CI runs, you merge, and Argo CD syncs the previous image. ECR keeps the last
10 images, and every pushed image counts, including releases you never
approved, so a rollback reaches back at most that far. Don't revert the very
first deploy commit: that would put back the placeholder image. The next
normal release still goes ahead, because the rolled-back tag is older than
the new one.

## Load test

```bash
ops/loadtest.sh                               # 50 users, 30 s, against http://<NODE_IP>
VUS=100 DURATION=2m ops/loadtest.sh
BASE_URL=http://localhost:8080 ops/loadtest.sh
```

`setup()` creates a lobby, then every virtual user GETs it as fast as it can.
The script prints total requests, success rate, requests per second and
p50/p95/p99 latency. It exits 0 when every request succeeded, 99 when any
failed, and 107 when it couldn't create the lobby.

A run over the internet from a WSL machine, against the m7i-flex.large node:

```
50 virtual users GET one lobby for 30s at http://<NODE_IP>
Requests:     36058
Success rate: 100.00% (0 failed)
Throughput:   1120.1 requests/s
Latency:      p50 36.9 ms   p95 69.6 ms   p99 101.4 ms
```

Most of that latency is the trip to Singapore. Prometheus measured the app
itself at p50 2.5 ms and p95 4.8 ms for the same requests.

### Zero-downtime check during a release

1. Start a long load test in one terminal:

   ```bash
   DURATION=5m ops/loadtest.sh
   ```

2. In another, release: approve a deploy, then make Argo CD sync at once:

   ```bash
   kubectl -n argocd annotate application root argocd.argoproj.io/refresh=normal --overwrite
   kubectl -n miniarcade rollout status deployment/miniarcade
   ```

3. The load test must end with `Success rate: 100.00% (0 failed)`.

Why it holds: the Deployment rolls with `maxUnavailable: 0`, so a new pod must
be Ready before an old one goes. The app's readiness needs Redis. On SIGTERM
the app keeps serving for 5 seconds while Kubernetes takes the pod out of the
Service, then finishes its in-flight requests.

Measured: a 4-minute run while Argo CD rolled the app over twice made 358,821
requests with a 100.00% success rate (0 failed, p95 49.5 ms). Each rollout
took about 8 seconds.

## Monitoring and alerts

- **Dashboard**: Grafana → **Dashboards → MiniArcade**: request rate, 5xx
  ratio, p95 latency, and games finished.
- **Logs**: Grafana → **Drilldown → Logs**, or **Explore → Loki**, for example
  `{app="miniarcade"} | json | status >= 400`. Loki keeps 3 days; its storage
  goes away with the cluster.
- **Alerts**, emailed to `<ALERT_EMAIL>` through Gmail (with a "resolved"
  email when they clear):
  - `MiniArcadeDown`: 0 available replicas for 2 minutes.
  - `MiniArcadeHighErrorRate`: over 5% of requests return 5xx for 5 minutes,
    while traffic is above 0.1 req/s.
  - `KubePodCrashLooping`: the built-in rule, for any pod.

  Every other alert, Watchdog included, goes to a null receiver.
- **Budget**: the AWS Budget emails at $10 and $20 of actual monthly cost. It
  is the one alert that also works while the cluster is down.

To check email delivery without breaking anything, send Alertmanager a test
alert:

```bash
kubectl -n monitoring port-forward svc/kube-prometheus-stack-alertmanager 9093 &
curl -XPOST localhost:9093/api/v2/alerts -H 'Content-Type: application/json' \
  -d '[{"labels":{"alertname":"MiniArcadeDown","namespace":"miniarcade","test":"true"},
        "annotations":{"summary":"TEST: not a real outage"}}]'
```

## Cost (ap-southeast-1)

Prices from the AWS Pricing API, November 2026:

| Item | Price | Running 24/7 (730 h) | Sessions: 2 h × 20 days (40 h) |
|---|---|---|---|
| m7i-flex.large, on demand (2 vCPU, 8 GiB) | $0.1197/h | $87.38 | $4.79 |
| 30 GB gp3 disk (exists only while the node does) | $0.096/GB-month | $2.88 | $0.16 |
| Public IPv4 address | $0.005/h | $3.65 | $0.20 |
| ECR storage (10 images of about 12 MB) | $0.10/GB-month | $0.01 | $0.01 |
| Data out (well under the free 100 GB/month), IAM, OIDC, budget | free | $0 | $0 |
| **Total** | | **≈ $93.92/month** | **≈ $5.16/month** |

A session costs about **$0.13 per hour** the cluster is up. A month with no
sessions costs about a cent. Running 24/7 would trigger the $20 budget emails
within the first week.

**Why not a t3.large?** The original plan was a t3.large ($0.1056/h) with
`cpu_credits = "standard"`, so it could never bill for extra CPU. But a t3 in
standard mode starts with no CPU credits, and this stack never lets it run
below its 30% baseline long enough to earn any. It stays capped at 0.6 of its
2 vCPUs. On it, the first sync took 15 minutes, the Kubernetes API stalled,
and the zero-downtime check lost 0.7% of requests. The m7i-flex.large costs
13% more, has no credits to run out, and never bills surplus CPU.

## Security

- Player tokens are 128-bit random values, compared in constant time and never
  logged.
- Lobby codes are format-checked before any Redis call, and request bodies are
  limited to 1 KiB.
- Every response carries `Content-Security-Policy: default-src 'self';
  frame-ancestors 'none'`, `X-Content-Type-Options: nosniff` and
  `Referrer-Policy: no-referrer`. The page writes values with `textContent`
  only.
- The image is distroless and non-root, with its base images pinned by digest.
  Pods run as non-root with a read-only root filesystem, all capabilities
  dropped and seccomp `RuntimeDefault`, enforced by Pod Security `restricted`.
- GitHub reaches AWS only through OIDC; there are no static keys in GitHub.
  The node pulls from ECR with its instance profile. IMDSv2 with hop limit 1
  keeps pods away from the node's credentials.
- Ports 22 and 6443 accept only `<MY_IP>`. Ports 9090, Argo CD and Grafana are
  never exposed.

**Accepted risks**: plain HTTP (tokens can be read on the network); one node,
so no high availability; no rate limiting (the Redis memory cap, the 1-hour
TTL and teardown limit the damage); anyone with a lobby link can watch.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `aws sso login` prints a `gio:` error | WSL can't open a browser. Use `--use-device-code --no-browser` and open the printed link yourself. |
| `cargo run` exits with "address in use" | Something on your machine already uses 9090. Run with `OPS_ADDR=127.0.0.1:19090`. |
| `docker stop` kills the app mid-drain | Docker Desktop allows 1 s by default. Use `docker stop -t 10`. |
| `docker login` to ECR fails with "The stub received bad data" | Docker Desktop's Windows credential store can't hold ECR tokens. Use a temporary `DOCKER_CONFIG` folder, or let the pipeline push. |
| Release: "Not authorized to perform sts:AssumeRoleWithWebIdentity" | The OIDC subject doesn't match. Compare `github_sub_prefix` with `gh api repos/<owner>/<repo>/actions/oidc/customization/sub`. |
| Alertmanager log: `535 5.7.8 Username and Password not accepted` | Gmail refused the app password: wrong account, revoked, or mistyped. Create a new one, update `secrets.yml`, re-run `ops/up.sh`. |
| Everything is slow, `kubectl` times out, the instance's CloudWatch CPU sits at exactly 30% | A burstable (t3) node has run out of CPU credits; see the cost section. Use the m7i-flex.large from `infra/cluster`. |
| CoreDNS keeps logging "No files matching import glob pattern" | Harmless. k3s imports an optional `coredns-custom` ConfigMap that doesn't exist. |
| A load test shows a few timeouts with no release going on | The internet path from WSL to Singapore can drop new connections. k6 reuses its connections, so this is rare; re-run, or test from inside the cluster to rule it out. |
