#!/usr/bin/env bash
# Fails if a benchmark misses its hard target (docs/ARCHITECTURE.md §13).
# Usage: scripts/check-bench-targets.sh bench.json
set -euo pipefail
f=$1
fail=0
check() { # name-regex max
  local v
  v=$(jq -r --arg re "$1" '[.[] | select(.name | test($re)) | .value] | max // empty' "$f")
  if [ -z "$v" ]; then echo "?? $1: no data"; return; fi
  if awk -v v="$v" -v m="$2" 'BEGIN { exit !(v > m) }'; then
    echo "FAIL $1: $v > $2"; fail=1
  else
    echo "ok   $1: $v <= $2"
  fi
}
check '^1 simulated day, 10000 agents$' 500
check '^1 simulated day, 100000 agents$' 5000
check '^1 simulated day, 1000000 agents$' 60000
check '^memory per agent' 384
check '^region checkpoint size, 10k agents$' 2048
check '^WASM catch-up, 1 day, region 0' 1500
exit $fail
