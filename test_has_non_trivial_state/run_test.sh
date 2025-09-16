#!/bin/bash

# Test script for WASM smart contracts
set -e

echo "Building WASM smart contract test..."
cargo build

git clone https://github.com/MathieuDutSik/linera-protocol_second
cd linera-protocol_second && cargo build && cd ..



echo "Linking linera binaries..."
export LINERA_PATH=$PWD/linera-protocol_second
ln -sf $LINERA_PATH/target/debug/linera target/debug/linera
ln -sf $LINERA_PATH/target/debug/linera-server target/debug/linera-server
ln -sf $LINERA_PATH/target/debug/linera-proxy target/debug/linera-proxy

echo "Running wasm test"
cargo run
