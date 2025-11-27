// SPDX-License-Identifier: GPL-2.0-or-later
pragma solidity ^0.8.0;

import "../lib/forge-std/src/Test.sol";
import "../lib/forge-std/src/console.sol";

import {IMorpho, MarketParams, Position, Market, Id} from "../src/interfaces/IMorpho.sol";
import {IrmMock} from "../src/mocks/IrmMock.sol";
import {ERC20Mock} from "../src/mocks/ERC20Mock.sol";
import {OracleMock} from "../src/mocks/OracleMock.sol";
import {Morpho} from "../src/Morpho.sol";
import {MarketParamsLib} from "../src/libraries/MarketParamsLib.sol";
import {MathLib} from "../src/libraries/MathLib.sol";
import {SharesMathLib} from "../src/libraries/SharesMathLib.sol";

/// @title SimpleNonReentrantTest
/// @notice Simple, standalone tests demonstrating Morpho Blue WITHOUT callbacks
/// @dev All functions called with hex"" to ensure NO REENTRANCY
contract SimpleNonReentrantTest is Test {
    using MarketParamsLib for MarketParams;
    using MathLib for uint256;

    // Contracts
    Morpho public morpho;
    ERC20Mock public loanToken;
    ERC20Mock public collateralToken;
    OracleMock public oracle;
    IrmMock public irm;

    // Users
    address public owner;
    address public supplier;
    address public borrower;
    address public liquidator;

    // Market
    MarketParams public marketParams;
    Id public id;

    // Constants
    uint256 constant ORACLE_PRICE_SCALE = 1e36;
    uint256 constant LLTV = 0.8 ether; // 80% loan-to-value

    function setUp() public {
        console.log("=== Setting up SimpleNonReentrantTest ===");

        // Create users
        owner = makeAddr("Owner");
        supplier = makeAddr("Supplier");
        borrower = makeAddr("Borrower");
        liquidator = makeAddr("Liquidator");

        // Deploy contracts
        morpho = new Morpho(owner);
        loanToken = new ERC20Mock();
        collateralToken = new ERC20Mock();
        oracle = new OracleMock();
        irm = new IrmMock();

        // Setup oracle price (1:1)
        oracle.setPrice(ORACLE_PRICE_SCALE);

        // Enable IRM and LLTV as owner
        vm.startPrank(owner);
        morpho.enableIrm(address(irm));
        morpho.enableLltv(LLTV);
        vm.stopPrank();

        // Create market
        marketParams = MarketParams({
            loanToken: address(loanToken),
            collateralToken: address(collateralToken),
            oracle: address(oracle),
            irm: address(irm),
            lltv: LLTV
        });
        morpho.createMarket(marketParams);
        id = marketParams.id();

        // Setup approvals
        vm.prank(supplier);
        loanToken.approve(address(morpho), type(uint256).max);

        vm.prank(borrower);
        loanToken.approve(address(morpho), type(uint256).max);
        vm.prank(borrower);
        collateralToken.approve(address(morpho), type(uint256).max);

        vm.prank(liquidator);
        loanToken.approve(address(morpho), type(uint256).max);

        console.log("Setup complete!");
    }

    /// @notice Test 1: Simple supply and withdraw (NO CALLBACKS)
    function test_SimpleSupplyWithdraw() public {
        console.log("\n=== Test 1: Simple Supply & Withdraw ===");

        uint256 supplyAmount = 1000 ether;

        // Give supplier tokens
        loanToken.setBalance(supplier, supplyAmount);
        console.log("Supplier balance:", supplyAmount / 1 ether, "tokens");

        // Supplier supplies (NO CALLBACK - empty data)
        vm.prank(supplier);
        morpho.supply(marketParams, supplyAmount, 0, supplier, hex"");
        console.log("Supplied:", supplyAmount / 1 ether, "tokens");

        // Check market state
        (uint128 totalSupplyAssets, uint128 totalSupplyShares, uint128 totalBorrowAssets, uint128 totalBorrowShares, uint128 lastUpdate, uint128 fee) = morpho.market(id);
        assertEq(totalSupplyAssets, supplyAmount, "Total supply mismatch");
        console.log("Total supply in market:", totalSupplyAssets / 1 ether, "tokens");

        // Withdraw half
        uint256 withdrawAmount = 500 ether;
        vm.prank(supplier);
        morpho.withdraw(marketParams, withdrawAmount, 0, supplier, supplier);
        console.log("Withdrew:", withdrawAmount / 1 ether, "tokens");

        // Verify
        assertEq(loanToken.balanceOf(supplier), withdrawAmount, "Withdrawal failed");
        console.log("Supplier now has:", loanToken.balanceOf(supplier) / 1 ether, "tokens");
        console.log(" Test 1 passed!");
    }

    /// @notice Test 2: Complete borrow/repay cycle (NO CALLBACKS)
    function test_CompleteBorrowRepayCycle() public {
        console.log("\n=== Test 2: Borrow/Repay Cycle ===");

        uint256 supplyAmount = 10000 ether;
        uint256 collateralAmount = 1000 ether;
        uint256 borrowAmount = 600 ether; // 75% of max (800)

        // Step 1: Supplier provides liquidity
        loanToken.setBalance(supplier, supplyAmount);
        vm.prank(supplier);
        morpho.supply(marketParams, supplyAmount, 0, supplier, hex"");
        console.log("1. Supplier deposited:", supplyAmount / 1 ether, "loan tokens");

        // Step 2: Borrower supplies collateral (NO CALLBACK)
        collateralToken.setBalance(borrower, collateralAmount);
        vm.prank(borrower);
        morpho.supplyCollateral(marketParams, collateralAmount, borrower, hex"");
        console.log("2. Borrower deposited:", collateralAmount / 1 ether, "collateral tokens");

        // Step 3: Borrower borrows
        vm.prank(borrower);
        morpho.borrow(marketParams, borrowAmount, 0, borrower, borrower);
        console.log("3. Borrower borrowed:", borrowAmount / 1 ether, "loan tokens");
        assertEq(loanToken.balanceOf(borrower), borrowAmount, "Borrow failed");

        // Step 4: Borrower repays (NO CALLBACK)
        vm.prank(borrower);
        morpho.repay(marketParams, borrowAmount, 0, borrower, hex"");
        console.log("4. Borrower repaid:", borrowAmount / 1 ether, "loan tokens");

        // Step 5: Borrower withdraws collateral
        vm.prank(borrower);
        morpho.withdrawCollateral(marketParams, collateralAmount, borrower, borrower);
        console.log("5. Borrower withdrew:", collateralAmount / 1 ether, "collateral tokens");

        // Verify final state
        assertEq(collateralToken.balanceOf(borrower), collateralAmount, "Collateral withdrawal failed");
        (,, uint128 totalBorrowAssets,,,) = morpho.market(id);
        assertEq(totalBorrowAssets, 0, "Debt not fully repaid");
        console.log(" Test 2 passed - Full cycle complete!");
    }

    /// @notice Test 3: Liquidation (NO CALLBACKS)
    function test_Liquidation() public {
        console.log("\n=== Test 3: Liquidation ===");

        uint256 supplyAmount = 10000 ether;
        uint256 collateralAmount = 1000 ether;
        uint256 borrowAmount = 700 ether; // Close to max

        // Setup position
        loanToken.setBalance(supplier, supplyAmount);
        vm.prank(supplier);
        morpho.supply(marketParams, supplyAmount, 0, supplier, hex"");

        collateralToken.setBalance(borrower, collateralAmount);
        vm.prank(borrower);
        morpho.supplyCollateral(marketParams, collateralAmount, borrower, hex"");

        vm.prank(borrower);
        morpho.borrow(marketParams, borrowAmount, 0, borrower, borrower);
        console.log("Position created: borrowed against collateral");

        // Price drops 20% - position becomes unhealthy
        oracle.setPrice(ORACLE_PRICE_SCALE * 80 / 100);
        console.log("Price dropped 20% - position is now unhealthy!");

        // Liquidator liquidates (NO CALLBACK)
        uint256 seizedAssets = 100 ether;
        loanToken.setBalance(liquidator, 1000 ether);
        vm.prank(liquidator);
        (uint256 seized, uint256 repaid) = morpho.liquidate(
            marketParams,
            borrower,
            seizedAssets,
            0,
            hex"" // NO CALLBACK
        );

        console.log("Liquidated - debt repaid:", repaid / 1 ether);
        console.log("Liquidated - collateral seized:", seized / 1 ether);
        assertEq(collateralToken.balanceOf(liquidator), seized, "Liquidation failed");
        console.log(" Test 3 passed - Liquidation successful!");
    }

    /// @notice Test 4: Interest accrual
    function test_InterestAccrual() public {
        console.log("\n=== Test 4: Interest Accrual ===");

        uint256 supplyAmount = 10000 ether;
        uint256 collateralAmount = 1000 ether;
        uint256 borrowAmount = 500 ether;

        // Setup position
        loanToken.setBalance(supplier, supplyAmount);
        vm.prank(supplier);
        morpho.supply(marketParams, supplyAmount, 0, supplier, hex"");

        collateralToken.setBalance(borrower, collateralAmount);
        vm.prank(borrower);
        morpho.supplyCollateral(marketParams, collateralAmount, borrower, hex"");

        vm.prank(borrower);
        morpho.borrow(marketParams, borrowAmount, 0, borrower, borrower);

        (,, uint128 totalBorrowAssetsBefore,,,) = morpho.market(id);
        console.log("Initial debt:", totalBorrowAssetsBefore / 1 ether, "tokens");

        // Time passes (1 year)
        vm.warp(block.timestamp + 365 days);
        morpho.accrueInterest(marketParams);

        (,, uint128 totalBorrowAssetsAfter,,,) = morpho.market(id);
        console.log("Debt after 1 year:", totalBorrowAssetsAfter / 1 ether, "tokens");

        assertGt(totalBorrowAssetsAfter, totalBorrowAssetsBefore, "Interest didn't accrue");
        uint256 interest = totalBorrowAssetsAfter - totalBorrowAssetsBefore;
        console.log("Interest accrued:", interest / 1 ether, "tokens");
        console.log(" Test 4 passed - Interest accrued!");
    }

    /// @notice Test 5: Multiple suppliers
    function test_MultipleSuppliers() public {
        console.log("\n=== Test 5: Multiple Suppliers ===");

        address supplier2 = makeAddr("Supplier2");
        vm.prank(supplier2);
        loanToken.approve(address(morpho), type(uint256).max);

        uint256 amount1 = 1000 ether;
        uint256 amount2 = 500 ether;

        // Supplier 1
        loanToken.setBalance(supplier, amount1);
        vm.prank(supplier);
        morpho.supply(marketParams, amount1, 0, supplier, hex"");
        console.log("Supplier 1 deposited:", amount1 / 1 ether, "tokens");

        // Supplier 2
        loanToken.setBalance(supplier2, amount2);
        vm.prank(supplier2);
        morpho.supply(marketParams, amount2, 0, supplier2, hex"");
        console.log("Supplier 2 deposited:", amount2 / 1 ether, "tokens");

        (uint128 totalSupplyAssets,,,,,) = morpho.market(id);
        assertEq(totalSupplyAssets, amount1 + amount2, "Total supply wrong");
        console.log("Total in pool:", totalSupplyAssets / 1 ether, "tokens");

        // Both can withdraw
        vm.prank(supplier);
        morpho.withdraw(marketParams, amount1, 0, supplier, supplier);
        console.log("Supplier 1 withdrew:", amount1 / 1 ether, "tokens");

        vm.prank(supplier2);
        morpho.withdraw(marketParams, amount2, 0, supplier2, supplier2);
        console.log("Supplier 2 withdrew:", amount2 / 1 ether, "tokens");

        console.log(" Test 5 passed - Multiple suppliers work!");
    }

    /// @notice Test 6: Maximum borrow capacity
    function test_MaxBorrowCapacity() public {
        console.log("\n=== Test 6: Max Borrow Capacity ===");

        uint256 supplyAmount = 10000 ether;
        uint256 collateralAmount = 1000 ether;
        uint256 maxBorrow = (collateralAmount * LLTV) / 1 ether; // 800 tokens

        console.log("Collateral:", collateralAmount / 1 ether, "tokens");
        console.log("LLTV: 80%");
        console.log("Max borrow:", maxBorrow / 1 ether, "tokens");

        // Setup
        loanToken.setBalance(supplier, supplyAmount);
        vm.prank(supplier);
        morpho.supply(marketParams, supplyAmount, 0, supplier, hex"");

        collateralToken.setBalance(borrower, collateralAmount);
        vm.prank(borrower);
        morpho.supplyCollateral(marketParams, collateralAmount, borrower, hex"");

        // Borrow close to max
        uint256 safeBorrow = maxBorrow - 1 ether;
        vm.prank(borrower);
        morpho.borrow(marketParams, safeBorrow, 0, borrower, borrower);
        console.log("Successfully borrowed:", safeBorrow / 1 ether, "tokens ");

        // Try to borrow more - should fail
        vm.prank(borrower);
        vm.expectRevert();
        morpho.borrow(marketParams, 2 ether, 0, borrower, borrower);
        console.log("Cannot borrow more - health check works! ");

        console.log(" Test 6 passed - Borrow limits enforced!");
    }
}
