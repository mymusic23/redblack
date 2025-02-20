#!/bin/bash
set -e

# Add wasm target if not already added
rustup target add wasm32-unknown-unknown

# Build the wasm package
cargo build -p redgold-gui --target wasm32-unknown-unknown

# Create wasm directory if it doesn't exist
mkdir -p vue-explorer/src/wasm

# Generate JavaScript bindings and copy WASM
wasm-bindgen --target web \
    --out-dir vue-explorer/src/wasm \
    target/wasm32-unknown-unknown/debug/redgold_gui.wasm

echo "WASM test environment setup complete"
echo "Now run: cd vue-explorer && npm run dev"
