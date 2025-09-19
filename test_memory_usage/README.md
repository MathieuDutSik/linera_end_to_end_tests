# Linera WASM Smart Contracts Testing

This project contains end-to-end tests for Linera smart contracts.

## Available Tests

### 1. Create and Call Test
Tests the create-and-call smart contract functionality.

### 2. Blob Access Test  
Tests the blob-access smart contract functionality:
- Publishes a blob to storage
- Stores the blob hash in the smart contract
- Retrieves blob content through the smart contract service
- Tests invalid blob hash access (should fail gracefully)
- Verifies stored blob hashes

## How to run

First, compile "linera-server", "linera-proxy" and "linera" and put links to them
in the directory "target/debug" (or "target/release" if that is your fancy).

Then run the tests:

```bash
# Run specific test
cargo run -- create-and-call
cargo run -- blob-access

# Run all tests
cargo run -- all
```
