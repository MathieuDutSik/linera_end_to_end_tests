#!/bin/bash

# Test script for running the fungible smart contracts.
set -e


cargo build


echo "Linking linera binaries..."
export LINERA_PATH="/Users/mathieudutoursikiric/GITlineraOpen/GITnull_graph_ql/linera-protocol_second"
ln -sf $LINERA_PATH/target/debug/linera target/debug/linera
ln -sf $LINERA_PATH/target/debug/linera-server target/debug/linera-server
ln -sf $LINERA_PATH/target/debug/linera-proxy target/debug/linera-proxy

echo "Running the complex-data tests"
cargo run complex-data
