#!/usr/bin/env bash
# Build the wasm module and assemble the static site in web/.
# Usage: ./build.sh [--serve [PORT]]
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript \
  --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/cascade_ca.wasm

echo "Built web/pkg/"

if [[ "${1:-}" == "--serve" ]]; then
  PORT="${2:-8080}"
  echo "Serving on http://localhost:${PORT}"
  python3 -m http.server "$PORT" --directory web
fi
