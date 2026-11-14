#!/usr/bin/env bash
# Load test with k6: setup() creates a lobby, then VUS virtual users GET it as
# fast as they can for DURATION. Prints total requests, success rate, requests
# per second and p50/p95/p99 latency. Any failed request fails the run, so it
# doubles as the zero-downtime check during a release (see the README).
#
#   ops/loadtest.sh                         # 50 users for 30 s against the cluster
#   VUS=100 DURATION=2m ops/loadtest.sh
#   BASE_URL=http://localhost:8080 ops/loadtest.sh
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VUS="${VUS:-50}"
DURATION="${DURATION:-30s}"
if [[ -z "${BASE_URL:-}" ]]; then
  export AWS_PROFILE="${AWS_PROFILE:-miniarcade}"
  if ! node_ip="$(terraform -chdir="$repo/infra/cluster" output -raw node_ip 2>/dev/null)"; then
    echo "No cluster to test: run ops/up.sh first, or set BASE_URL." >&2
    exit 1
  fi
  BASE_URL="http://$node_ip"
fi
echo "$VUS virtual users GET one lobby for $DURATION at $BASE_URL"

k6 run --quiet -e BASE_URL="$BASE_URL" -e VUS="$VUS" -e DURATION="$DURATION" - <<'EOF'
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  vus: Number(__ENV.VUS),
  duration: __ENV.DURATION,
  summaryTrendStats: ['p(50)', 'p(95)', 'p(99)'],
  // Even one failed request fails the run (exit code 99).
  thresholds: { checks: ['rate==1'] },
};

export function setup() {
  const res = http.post(`${__ENV.BASE_URL}/api/lobbies`);
  if (res.status !== 201) {
    throw new Error(`couldn't create a lobby: HTTP ${res.status}`);
  }
  return { url: `${__ENV.BASE_URL}/api/lobbies/${res.json('code')}` };
}

export default function (data) {
  const res = http.get(data.url);
  check(res, { 'status is 200': (r) => r.status === 200 });
}

export function handleSummary(data) {
  const m = data.metrics;
  const ok = m.checks ? m.checks.values.passes : 0;
  const failed = m.checks ? m.checks.values.fails : 0;
  const total = ok + failed;
  if (total === 0) {
    return { stdout: 'No requests ran: setup() failed (see the error above).\n' };
  }
  const ms = (name) => `${m.http_req_duration.values[name].toFixed(1)} ms`;
  const lines = [
    `Requests:     ${total}`,
    `Success rate: ${((100 * ok) / total).toFixed(2)}% (${failed} failed)`,
    `Throughput:   ${m.http_reqs.values.rate.toFixed(1)} requests/s`,
    `Latency:      p50 ${ms('p(50)')}   p95 ${ms('p(95)')}   p99 ${ms('p(99)')}`,
  ];
  return { stdout: `${lines.join('\n')}\n` };
}
EOF
