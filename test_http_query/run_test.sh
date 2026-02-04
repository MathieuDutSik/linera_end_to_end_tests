#!/bin/bash

# Test script for WASM smart contracts
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
cargo build


if [ ! -d "linera-protocol_second_branch_test_http_request" ]; then
    git clone https://github.com/MathieuDutSik/linera-protocol_second linera-protocol_second_branch_test_http_request
    cd linera-protocol_second_branch_test_http_request && git checkout test_http_request && cd ..
else
    echo "Directory already exists, skipping clone."
fi
cd linera-protocol_second_branch_test_http_request && git pull && cargo build && cd ..



echo "Linking linera binaries..."
export LINERA_PATH=$PWD/linera-protocol_second_branch_test_http_request
ln -sf $LINERA_PATH/target/debug/linera target/debug/linera
ln -sf $LINERA_PATH/target/debug/linera-server target/debug/linera-server
ln -sf $LINERA_PATH/target/debug/linera-proxy target/debug/linera-proxy

echo "Running wasm test"
cargo run
