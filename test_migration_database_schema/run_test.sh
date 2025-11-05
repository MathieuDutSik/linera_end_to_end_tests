#!/bin/bash

# Test script for running the fungible smart contracts.
set -e

echo "Building WASM smart contract tester."
cargo build

if [ ! -d "linera-protocol_test_conway_old_schema" ]; then
    git clone https://github.com/linera-io/linera-protocol linera-protocol_test_conway_old_schema
    cd linera-protocol_test_conway_old_schema && git checkout testnet_conway && cd ..
else
    echo "Directory already exists, skipping clone."
fi
cd linera-protocol_test_conway_old_schema && cargo build --features scylladb && cd ..


if [ ! -d "linera-protocol_test_conway_new_schema" ]; then
    git clone https://github.com/MathieuDutSik/linera-protocol_second linera-protocol_test_conway_new_schema
    cd linera-protocol_test_conway_new_schema && git checkout introduce_new_schema_and_migration_tool && cd ..
else
    echo "Directory already exists, skipping clone."
fi
cd linera-protocol_test_conway_new_schema && cargo build --features scylladb && cd ..

rm -f LOG_server* LOG_proxy*
echo "Running the social test"
cargo run social

