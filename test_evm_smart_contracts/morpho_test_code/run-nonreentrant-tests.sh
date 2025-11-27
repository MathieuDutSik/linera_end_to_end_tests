#!/bin/bash

# Script to run non-reentrant tests for Morpho Blue
# This demonstrates that Morpho works perfectly WITHOUT callbacks

set -e

echo "════════════════════════════════════════════════════════════"
echo "  Morpho Blue - Non-Reentrant Tests"
echo "  Testing full functionality WITHOUT callbacks"
echo "════════════════════════════════════════════════════════════"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Running all non-reentrant tests...${NC}"
echo ""

# Run all tests with verbose output
forge test --match-contract SimpleNonReentrantTest -vv

echo ""
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  All tests passed! ✅${NC}"
echo -e "${GREEN}  Morpho Blue works perfectly without reentrancy!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
