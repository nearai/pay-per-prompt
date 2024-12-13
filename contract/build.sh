#!/bin/bash
cargo build --target wasm32-unknown-unknown --release
rm -rf build
mkdir -p build
cp target/wasm32-unknown-unknown/release/payment_channel.wasm build/
