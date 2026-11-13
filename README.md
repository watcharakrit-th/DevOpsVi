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
