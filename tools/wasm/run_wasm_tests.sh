#!/usr/bin/env bash
# Runs the core's differential tests compiled to wasm32-wasip1 (32-bit
# `usize`, IEEE f64 -- the arithmetic of the shipped wasm32 build) under
# Node's WASI.  Needs `rustup target add wasm32-wasip1` and node >= 20.
#
#   tools/wasm/run_wasm_tests.sh [test-name ...]
#
# Tests that need threads (tests/robustness.rs) cannot run on wasip1.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
tests=("$@")
if [ ${#tests[@]} -eq 0 ]; then
  tests=(fixtures_ops audit_scenes nudging_oracle hyperedge_oracle objects_oracle orthogonal_vis_oracle
         visibility_oracle crossings_oracle geometry_oracle fixtures_orthogonal fixtures_polyline
         fixtures_hyperedge core_findings)
fi
export CARGO_TARGET_WASM32_WASIP1_RUNNER="node $ROOT/tools/wasm/wasi_runner.cjs"
args=()
for t in "${tests[@]}"; do args+=(--test "$t"); done
cargo test --release --target wasm32-wasip1 "${args[@]}" 2>&1 | grep -E "^test result|^     Running|FAILED|panicked|error"
