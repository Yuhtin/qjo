#!/bin/sh
# Compila o qjo-wasm e copia para public/qjo.wasm.
# Sem cargo (por exemplo, numa máquina de build sem Rust), usa o public/qjo.wasm que já existir.
set -e
cd "$(dirname "$0")/.."
SAIDA=public/qjo.wasm
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if command -v cargo >/dev/null 2>&1; then
  (cd .. && cargo build -p qjo-wasm --target wasm32-unknown-unknown --profile release-wasm)
  mkdir -p public
  cp ../target/wasm32-unknown-unknown/release-wasm/qjo_wasm.wasm "$SAIDA"
  echo "qjo.wasm atualizado ($(wc -c < "$SAIDA" | tr -d ' ') bytes)"
elif [ -f "$SAIDA" ]; then
  echo "cargo não encontrado; usando o $SAIDA existente"
else
  echo "cargo não encontrado e $SAIDA não existe. Instale o Rust (rustup target add wasm32-unknown-unknown)." >&2
  exit 1
fi
