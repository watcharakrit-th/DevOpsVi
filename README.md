# MiniArcade

A small DevOps learning lab: a two-player XO game in the browser, taken
through the whole DevOps cycle (plan, code, build, test, release, deploy,
monitor, operate) on AWS.

Work is tracked as GitHub Issues, one per build phase, on the MiniArcade
project board. The full README (setup from zero, daily up/down, release,
rollback, costs) arrives in phase 8.

## Run it locally

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

Open http://localhost:8080, start a game, and open the invite link in a
second tab to play the other side. The page is compiled into the binary, so
restart `cargo run` after editing files in `app/static/`.

Build and run the container image (distroless, runs as UID 65532):

```bash
docker build --tag miniarcade app
docker run --rm -p 8080:8080 -e REDIS_URL=redis://host.docker.internal:6379 miniarcade
```

The app drains for 5 s on SIGTERM, so stop it with `docker stop -t 10`; a
shorter timeout kills it mid-drain.

## AWS, once

This needs the AWS CLI v2 with an IAM Identity Center profile named
`miniarcade` (region ap-southeast-1). Log in with
`aws sso login --profile miniarcade`.

1. Put the alert address in a local-only file:

   ```bash
   mkdir -p -m 700 ~/.miniarcade
   (umask 077; echo 'alert_email = "<ALERT_EMAIL>"' > ~/.miniarcade/bootstrap.tfvars)
   ```

2. Apply the bootstrap root, once. It creates the ECR repository, the GitHub
   OIDC role and the $20 budget:

   ```bash
   cd infra/bootstrap
   export AWS_PROFILE=miniarcade
   terraform init
   terraform apply -var-file="$HOME/.miniarcade/bootstrap.tfvars"
   ```

3. Give the release pipeline its three repository variables:

   ```bash
   gh variable set AWS_REGION   --body "$(terraform output -raw aws_region)"
   gh variable set AWS_ROLE_ARN --body "$(terraform output -raw aws_role_arn)"
   gh variable set ECR_REGISTRY --body "$(terraform output -raw ecr_registry)"
   ```

4. Give Alertmanager a way to send email. Create an app password for the
   sending Gmail account at https://myaccount.google.com/apppasswords (it
   needs 2-Step Verification), then fill in `~/.miniarcade/secrets.yml`
   (`chmod 600`):

   ```yaml
   gmail_address: "<sending Gmail address>"
   gmail_app_password: "<app password>"
   alert_email: "<ALERT_EMAIL>"
   ```

   `ops/up.sh` refuses to start without it. Ansible turns it into a Kubernetes
   Secret and never prints it.

## Each session

```bash
aws sso login --profile miniarcade
ops/up.sh      # creates the node and k3s; prints http://<NODE_IP>/ and the kubectl setup
ops/down.sh    # destroys the cluster; bootstrap stays
```

`ops/up.sh` is safe to re-run, for example when your IP changes: SSH and the
Kubernetes API accept only `<MY_IP>`. Its first run creates an SSH key and
`~/.miniarcade/cluster.tfvars`.
