#!/usr/bin/env bash
# Recomputes the golden state hash after an intentional change to the simulation.
set -euo pipefail
cd "$(dirname "$0")/.."
h=$(LIVINGSIM_PRINT_GOLDEN=1 cargo test --release -q -p livingsim-core --test determinism golden_hash -- --nocapture 2>/dev/null | sed -n 's/^GOLDEN=//p')
[ -n "$h" ] || { echo "could not compute golden hash" >&2; exit 1; }
echo "$h" > crates/livingsim-core/tests/golden.txt
echo "golden hash: $h"
