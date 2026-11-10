#!/bin/sh
# Bygger webbversionen i dist/web: index.html + krabba_spelet.js (WebAssembly som base64).
# Kräver: rustup target add wasm32-unknown-unknown
set -e
cd "$(dirname "$0")/.."
cargo build --release --lib --target wasm32-unknown-unknown
W=target/wasm32-unknown-unknown/release/krabba_spelet.wasm
if command -v wasm-opt >/dev/null 2>&1; then wasm-opt -O3 "$W" -o target/krabba_spelet.opt.wasm && W=target/krabba_spelet.opt.wasm; fi
rm -rf dist/web && mkdir -p dist/web
cp web/index.html dist/web/
cp "$W" dist/web/krabba_spelet.wasm
printf 'window.KRABBA_WASM_B64="' > dist/web/krabba_spelet.js
base64 -w0 "$W" >> dist/web/krabba_spelet.js
printf '";\n' >> dist/web/krabba_spelet.js
cp assets/fonts/OFL-*.txt dist/web/
echo "Klart: dist/web (öppna index.html)"
