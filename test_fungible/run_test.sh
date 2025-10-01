#!/bin/bash

# Test script for running the fungible smart contracts.
set -e

export STORAGE_SERVICE_PORT=1235


if netstat -an 2>/dev/null | grep -q "[.:]$STORAGE_SERVICE_PORT[[:space:]]" ; then
    echo "A storage service is apparently running on $STORAGE_SERVICE_PORT. Let us continue"
else
    echo "No one is listering on $STORAGE_SERVICE_PORT."
    echo "No storage service running. Please run one with the command"
    echo "cargo run --release -p linera-storage-service -- memory --endpoint 127.0.0.1:1235"
    echo "Exisint"
    exit 1
fi


echo "Building WASM smart contract tester."
cargo build --release

if [ ! -d "linera-protocol" ]; then
    git clone https://github.com/linera-io/linera-protocol
else
    echo "Directory already exists, skipping clone."
fi
cd linera-protocol && cargo build --release && cd ..



echo "Linking linera binaries..."
export LINERA_PATH=$PWD/linera-protocol
ln -sf $LINERA_PATH/target/release/linera target/release/linera
ln -sf $LINERA_PATH/target/release/linera-server target/release/linera-server
ln -sf $LINERA_PATH/target/release/linera-proxy target/release/linera-proxy

echo "Running the fungible tests"
cargo run --release all > output
cat output
