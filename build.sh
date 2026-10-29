#!/usr/bin/env bash
# WebAssembly をビルドして www/pkg に出力する。
#   必要なもの: rustup target add wasm32-unknown-unknown
#               cargo install wasm-bindgen-cli --version 0.2.129
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir www/pkg \
  target/wasm32-unknown-unknown/release/wedding_invitation.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -Os -o www/pkg/wedding_invitation_bg.wasm www/pkg/wedding_invitation_bg.wasm
fi
ls -lh www/pkg
