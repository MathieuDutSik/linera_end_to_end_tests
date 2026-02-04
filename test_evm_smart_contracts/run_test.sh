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
echo "Running morpho_supply_withdraw"
cargo run morpho_supply_withdraw > res_0 2>&1
if ! grep -q "Successful end" res_0; then
    echo "ERROR: morpho_supply_withdraw test failed - res_0 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_borrow_repay"
cargo run morpho_borrow_repay > res_1 2>&1
if ! grep -q "Successful end" res_1; then
    echo "ERROR: morpho_borrow_repay test failed - res_1 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_liquidation"
cargo run morpho_liquidation > res_2 2>&1
if ! grep -q "Successful end" res_2; then
    echo "ERROR: morpho_liquidation test failed - res_2 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_interest"
cargo run morpho_interest > res_3 2>&1
if ! grep -q "Successful end" res_3; then
    echo "ERROR: morpho_interest test failed - res_3 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_multiple_suppliers"
cargo run morpho_multiple_suppliers > res_4 2>&1
if ! grep -q "Successful end" res_4; then
    echo "ERROR: morpho_multiple_suppliers test failed - res_4 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_max_borrow"
cargo run morpho_max_borrow > res_5 2>&1
if ! grep -q "Successful end" res_5; then
    echo "ERROR: morpho_max_borrow test failed - res_5 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_supply_callback"
cargo run morpho_supply_callback > res_6 2>&1
if ! grep -q "Successful end" res_6; then
    echo "ERROR: morpho_supply_callback test failed - res_6 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_supply_collateral_callback"
cargo run morpho_supply_collateral_callback > res_7 2>&1
if ! grep -q "Successful end" res_7; then
    echo "ERROR: morpho_supply_collateral_callback test failed - res_7 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_repay_callback"
cargo run morpho_repay_callback > res_8 2>&1
if ! grep -q "Successful end" res_8; then
    echo "ERROR: morpho_repay_callback test failed - res_8 does not contain 'Successful end'"
    exit 1
fi

echo "Running morpho_liquidate_callback"
cargo run morpho_liquidate_callback > res_9 2>&1
if ! grep -q "Successful end" res_9; then
    echo "ERROR: morpho_liquidate_callback test failed - res_9 does not contain 'Successful end'"
    exit 1
fi

#cargo run morpho_supply_collateral_callback > res 2>&1

echo "EVM test completed successfully!"
