#!/bin/bash

# Test script for EVM smart contracts
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

cd /Users/mathieudutoursikiric/GITlineraOpen/GITevm_morpho_related_issues/linera-protocol_second && cargo build --features revm && cd /Users/mathieudutoursikiric/GITall/GITmathieu/linera_end_to_end_tests/test_evm_smart_contracts

cd morpho_test_code && ./solc-0.8.19 --standard-json < config.json > result.out && cd ..

echo "Building EVM smart contract test..."
cargo build




echo "Linking linera binaries..."

export LINERA_PATH=/Users/mathieudutoursikiric/GITlineraOpen/GITevm_morpho_related_issues/linera-protocol_second
ln -sf $LINERA_PATH/target/debug/linera target/debug/linera
ln -sf $LINERA_PATH/target/debug/linera-server target/debug/linera-server
ln -sf $LINERA_PATH/target/debug/linera-proxy target/debug/linera-proxy

echo "Running some EVM tests..."
#cargo run evm-counter
cargo run morpho_not_reentrant > res 2>&1

echo "EVM test completed successfully!"
