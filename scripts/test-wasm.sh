#!/usr/bin/env bash
# Builds the WASM bindings for Node and checks native/WASM determinism + catch-up time.
# Requires: wasm32-unknown-unknown target, wasm-bindgen-cli (same version as Cargo.lock), node.
set -euo pipefail
cd "$(dirname "$0")/.."
SEED=${SEED:-42}
TICKS=${TICKS:-1440}
cargo build --release -p livingsim-runner
cargo build --release --target wasm32-unknown-unknown -p livingsim-wasm
wasm-bindgen --target nodejs --out-dir target/wasm-node target/wasm32-unknown-unknown/release/livingsim_wasm.wasm
EXPECTED=$(target/release/livingsim-runner hash --seed "$SEED" --ticks "$TICKS")
WORLD=target/wasm-test-world
rm -rf "$WORLD"
# A fresh world with two consecutive checkpoints (lead of 1 hour = 1 simulated day).
target/release/livingsim-runner tick --out "$WORLD" --epoch-ms 1800000000000 --now-ms 1800000000000 --lead-hours 1 --seed "$SEED" >/dev/null 2>&1
node tests/wasm/determinism.mjs target/wasm-node "$EXPECTED" "$SEED" "$TICKS" "$WORLD"
