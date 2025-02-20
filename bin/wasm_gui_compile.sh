#!/bin/bash
set -e

# Add wasm target if not already added
rustup target add wasm32-unknown-unknown

# Install wasm-bindgen-cli if not already installed
cargo install wasm-bindgen-cli

# Build the wasm package
cargo build -p redgold-gui --target wasm32-unknown-unknown

# Generate JavaScript bindings
wasm-bindgen --target web --out-dir vue-explorer/src/wasm target/wasm32-unknown-unknown/debug/redgold_gui.wasm
