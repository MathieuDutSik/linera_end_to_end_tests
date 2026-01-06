use anyhow::Result;
use alloy_primitives::{U256, Address};
use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
use linera_base::vm::{EvmInstantiation, EvmOperation, EvmQuery};
use linera_sdk::{
    abis::evm::EvmAbi,
    linera_base_types::{Account, Amount, ApplicationId},
};
use std::{
    str::FromStr,
    path::PathBuf,
};

mod solidity;
use solidity::{read_evm_address_entry, read_and_publish_contract};

use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use std::env;

#[derive(Debug, Clone)]
struct MarketParamsData {
    loan_token: Address,
    collateral_token: Address,
    oracle: Address,
    irm: Address,
    lltv: U256,
}

fn get_zero_operation(operation: impl alloy_sol_types::SolCall) -> Result<EvmQuery, bcs::Error> {
    let operation = EvmOperation::new(Amount::ZERO, operation.abi_encode());
    operation.to_evm_query()
}

/// Parse a JSON array response into a fixed-size byte array
fn parse_bytes32_from_array(value: &serde_json::Value) -> Result<[u8; 32]> {
    let array = value.as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected array response"))?;
    let mut bytes = [0u8; 32];
    for (i, byte_val) in array.iter().enumerate() {
        if i < 32 {
            bytes[i] = byte_val.as_u64()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse byte value at index {}", i))? as u8;
        }
    }
    Ok(bytes)
}

/// Parse a uint256 value from a JSON array response
fn parse_u256_from_array(value: &serde_json::Value) -> Result<U256> {
    let bytes = parse_bytes32_from_array(value)?;
    Ok(U256::from_be_bytes(bytes))
}

/// Parse a uint128 value from a specific offset in a JSON array response
/// offset: the starting byte position in the array (e.g., 0 for first value, 32 for second, etc.)
fn parse_u128_from_array_at_offset(value: &serde_json::Value, offset: usize) -> Result<u128> {
    let array = value.as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected array response"))?;

    // uint128 is 16 bytes, right-aligned in a 32-byte word
    // So for offset N, we need bytes [N+16..N+32]
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        let array_idx = offset + 16 + i;
        if array_idx < array.len() {
            bytes[i] = array[array_idx].as_u64()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse byte at index {}", array_idx))? as u8;
        }
    }
    Ok(u128::from_be_bytes(bytes))
}

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
    config
}


async fn test_evm_end_to_end_morpho_not_reentrant(choice: usize) -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    let config = get_config();

    tracing::info!("Starting EVM Morpho non-reentrant end-to-end test");
    // Creating the clients and multi-owner chain
    let (mut net, client_regular) = config.instantiate().await?;

    // Create additional clients for each user
    let client_owner = net.make_client().await;
    client_owner.wallet_init(None).await?;

    let client_supplier = net.make_client().await;
    client_supplier.wallet_init(None).await?;

    let client_borrower = net.make_client().await;
    client_borrower.wallet_init(None).await?;

    let client_liquidator = net.make_client().await;
    client_liquidator.wallet_init(None).await?;

    let client_supplier2 = net.make_client().await;
    client_supplier2.wallet_init(None).await?;

    let chain1 = *client_regular.load_wallet()?.owned_chain_ids().first().unwrap();

    // Generate keys for all clients
    let owner_regular = client_regular.keygen().await?;
    let owner_owner = client_owner.keygen().await?;
    let owner_supplier = client_supplier.keygen().await?;
    let owner_borrower = client_borrower.keygen().await?;
    let owner_liquidator = client_liquidator.keygen().await?;
    let owner_supplier2 = client_supplier2.keygen().await?;

    // Open a chain owned by all six users
    let chain2 = client_regular
        .open_multi_owner_chain(
            chain1,
            vec![owner_regular, owner_owner, owner_supplier, owner_borrower, owner_liquidator, owner_supplier2],
            vec![100, 100, 100, 100, 100, 100],
            u32::MAX,
            Amount::from_tokens(1000),
            10_000,
        )
        .await?;

    // Assign chain2 to all clients
    client_regular.assign(owner_regular, chain2).await?;
    client_owner.assign(owner_owner, chain2).await?;
    client_supplier.assign(owner_supplier, chain2).await?;
    client_borrower.assign(owner_borrower, chain2).await?;
    client_liquidator.assign(owner_liquidator, chain2).await?;
    client_supplier2.assign(owner_supplier2, chain2).await?;

    let account1 = Account {
        chain_id: chain2,
        owner: owner_regular,
    };
    let account_chain = Account::chain(chain2);
    client_regular
        .transfer_with_accounts(Amount::from_tokens(800), account_chain, account1)
        .await?;

    assert_eq!(client_regular.local_balance(account_chain).await?, Amount::from_micros(199999990));
    assert_eq!(client_regular.local_balance(account1).await?, Amount::from_tokens(800));
    assert_eq!(client_regular.query_balance(account_chain).await?, Amount::from_micros(199999990));
    assert_eq!(client_regular.query_balance(account1).await?, Amount::from_tokens(800));

    sol! {
        struct MarketParams {
            address loanToken;
            address collateralToken;
            address oracle;
            address irm;
            uint256 lltv;
        }

        function test_SimpleSupplyWithdraw();
        function set_addresses(
            address ownerAddress,
            address supplierAddress,
            address borrowerAddress,
            address liquidatorAddress,
            address supplier2Address
        );
        function set_up_part_a();
        function set_up_part_a2();
        function set_up_part_b();
        function set_up_part_c();
        function set_up_part_d();
        function set_up_part_e();
        function get_irm();
        function get_oracle();
        function get_morpho();
        function get_loan_token();
        function get_collateral_token();
        function id() external view returns (bytes32);
        function enableIrm(address irm);
        function enableLltv(uint256 lltv);
        function approve(address spender, uint256 amount);
        function setBalance(address owner, uint256 amount);
        function supply(
            MarketParams marketParams,
            uint256 assets,
            uint256 shares,
            address onBehalf,
            bytes data
        ) external returns (uint256, uint256);
        function withdraw(
            MarketParams marketParams,
            uint256 assets,
            uint256 shares,
            address onBehalf,
            address receiver
        ) external returns (uint256, uint256);
        function market(bytes32 id) external view returns (
            uint128 totalSupplyAssets,
            uint128 totalSupplyShares,
            uint128 totalBorrowAssets,
            uint128 totalBorrowShares,
            uint128 lastUpdate,
            uint128 fee
        );
        function balanceOf(address owner) external view returns (uint256);
    }

    println!("test_evm_end_to_end_morpho_not_reentrant, step 1 - Deploying contracts");
    let path = PathBuf::from("morpho_test_code/result.out");
    println!("test_evm_end_to_end_morpho_not_reentrant, step 2");

    let constructor_argument = Vec::new();

    let start_value = Amount::from_tokens(100);
    let evm_instantiation = EvmInstantiation {
        value: start_value.into(),
        argument: vec![],
    };

    println!("test_evm_end_to_end_morpho_not_reentrant, step 3 - Extracting EVM addresses");

    // Extract EVM addresses from all account owners
    let address_regular = owner_regular.to_evm_address().unwrap();
    let address_owner = owner_owner.to_evm_address().unwrap();
    let address_supplier = owner_supplier.to_evm_address().unwrap();
    let address_borrower = owner_borrower.to_evm_address().unwrap();
    let address_liquidator = owner_liquidator.to_evm_address().unwrap();
    let address_supplier2 = owner_supplier2.to_evm_address().unwrap();

    println!("address_regular: {:?}", address_regular);
    println!("address_owner: {:?}", address_owner);
    println!("address_supplier: {:?}", address_supplier);
    println!("address_borrower: {:?}", address_borrower);
    println!("address_liquidator: {:?}", address_liquidator);
    println!("address_supplier2: {:?}", address_supplier2);

    // Deploy SimpleNonReentrantTest
    println!("test_evm_end_to_end_morpho_not_reentrant, step 4 - Deploying test contract (SimpleNonReentrantTest)");
    let test_contract_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "SimpleNonReentrantTest.sol",
        "SimpleNonReentrantTest",
        constructor_argument.clone(),
        evm_instantiation,
        Some(chain2),
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, test_contract_app_id={:?}", test_contract_app_id);

    let test_contract_address = test_contract_app_id.evm_address();
    println!("test_contract_address: {:?}", test_contract_address);

    // Create node services for all clients
    let port_regular = get_node_port().await;
    let port_owner = get_node_port().await;
    let port_supplier = get_node_port().await;
    let port_borrower = get_node_port().await;
    let port_liquidator = get_node_port().await;
    let port_supplier2 = get_node_port().await;

    let mut node_service_regular = client_regular.run_node_service(port_regular, ProcessInbox::Skip).await?;
    let mut node_service_owner = client_owner.run_node_service(port_owner, ProcessInbox::Skip).await?;
    let mut node_service_supplier = client_supplier.run_node_service(port_supplier, ProcessInbox::Skip).await?;
    let mut node_service_borrower = client_borrower.run_node_service(port_borrower, ProcessInbox::Skip).await?;
    let mut node_service_liquidator = client_liquidator.run_node_service(port_liquidator, ProcessInbox::Skip).await?;
    let mut node_service_supplier2 = client_supplier2.run_node_service(port_supplier2, ProcessInbox::Skip).await?;

    println!("test_evm_end_to_end_morpho_not_reentrant, step 8 - Creating application wrappers");

    // Create test contract application wrappers for each user
    let test_contract_regular = node_service_regular.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_owner = node_service_owner.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_supplier = node_service_supplier.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_borrower = node_service_borrower.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_liquidator = node_service_liquidator.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_supplier2 = node_service_supplier2.make_application(&chain2, &test_contract_app_id)?;

    println!("test_evm_end_to_end_morpho_not_reentrant, step 9 - All application wrappers created");

    // Step 1: Set user addresses
    println!("test_evm_end_to_end_morpho_not_reentrant, step 11 - Setting user addresses");
    let operation = set_addressesCall {
        ownerAddress: address_owner,
        supplierAddress: address_supplier,
        borrowerAddress: address_borrower,
        liquidatorAddress: address_liquidator,
        supplier2Address: address_supplier2,
    };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 12 - User addresses set");

    // Step 2: Deploy all contracts and initialize (set_up_part_a)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 13 - Running set_up_part_a");
    let operation = set_up_part_aCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 14 - set_up_part_a completed");

    let query = get_irmCall { };
    let query = EvmQuery::Query(query.abi_encode());
    let irm = test_contract_regular.run_json_query(query).await?;
    let irm = read_evm_address_entry(irm);

    let query = get_oracleCall { };
    let query = EvmQuery::Query(query.abi_encode());
    let oracle = test_contract_regular.run_json_query(query).await?;
    let oracle = read_evm_address_entry(oracle);

    let query = get_morphoCall { };
    let query = EvmQuery::Query(query.abi_encode());
    let morpho = test_contract_regular.run_json_query(query).await?;
    let morpho = read_evm_address_entry(morpho);

    let morpho_id = ApplicationId::from(morpho).with_abi::<EvmAbi>();
    let morpho_regular = node_service_regular.make_application(&chain2, &morpho_id)?;
    let morpho_owner = node_service_owner.make_application(&chain2, &morpho_id)?;
    let morpho_supplier = node_service_supplier.make_application(&chain2, &morpho_id)?;
    let morpho_borrower = node_service_borrower.make_application(&chain2, &morpho_id)?;
    let morpho_liquidator = node_service_liquidator.make_application(&chain2, &morpho_id)?;
    let morpho_supplier2 = node_service_supplier2.make_application(&chain2, &morpho_id)?;

    // Step 3: Enable IRM
    println!("test_evm_end_to_end_morpho_not_reentrant, step 15 - Running enableIrm");
    let operation = enableIrmCall { irm };
    let operation = get_zero_operation(operation)?;
    node_service_owner.process_inbox(&chain2).await?;
    morpho_owner.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 16 - enableIrm completed");

    println!("test_evm_end_to_end_morpho_not_reentrant, step 17 - Running enableLltv");
    let lltv = U256::from_str("800000000000000000")?;
    let operation = enableLltvCall { lltv };
    let operation = get_zero_operation(operation)?;
    node_service_owner.process_inbox(&chain2).await?;
    morpho_owner.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 18 - enableLltv completed");

    // Step 4: Create market
    println!("test_evm_end_to_end_morpho_not_reentrant, step 19 - Running set_up_part_c");
    let operation = set_up_part_cCall { };
    let operation = get_zero_operation(operation)?;
    node_service_regular.process_inbox(&chain2).await?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 20 - set_up_part_c completed");

    // Step 5: Approve loan token (for all users)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 21 - Running set_up_part_d for all users");

    let query = get_loan_tokenCall { };
    let query = EvmQuery::Query(query.abi_encode());
    let loan_token = test_contract_regular.run_json_query(query).await?;
    let loan_token = read_evm_address_entry(loan_token);
    let loan_token_id = ApplicationId::from(loan_token).with_abi::<EvmAbi>();
    let loan_token_supplier = node_service_supplier.make_application(&chain2, &loan_token_id)?;
    let loan_token_borrower = node_service_borrower.make_application(&chain2, &loan_token_id)?;
    let loan_token_liquidator = node_service_liquidator.make_application(&chain2, &loan_token_id)?;
    let loan_token_regular = node_service_regular.make_application(&chain2, &loan_token_id)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - getting loan_token and applications");

    let amount = U256::MAX;
    let operation = approveCall { spender: morpho, amount };
    let operation = get_zero_operation(operation)?;
    node_service_supplier.process_inbox(&chain2).await?;
    loan_token_supplier.run_json_query(operation.clone()).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - done for supplier");
    node_service_borrower.process_inbox(&chain2).await?;
    loan_token_borrower.run_json_query(operation.clone()).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - done for borrower");
    node_service_liquidator.process_inbox(&chain2).await?;
    loan_token_liquidator.run_json_query(operation.clone()).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - done for liquidator");

    let query = get_collateral_tokenCall { };
    let query = EvmQuery::Query(query.abi_encode());
    let collateral_token = test_contract_regular.run_json_query(query).await?;
    let collateral_token = read_evm_address_entry(collateral_token);
    println!("test_evm_end_to_end_morpho_not_reentrant, step 23 - getting collateral_token and applications");
    let collateral_token_id = ApplicationId::from(collateral_token).with_abi::<EvmAbi>();
    let collateral_token_borrower = node_service_borrower.make_application(&chain2, &collateral_token_id)?;
    node_service_borrower.process_inbox(&chain2).await?;
    collateral_token_borrower.run_json_query(operation.clone()).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 23 - done for borrower");

    // Construct MarketParams
    let market_params = MarketParamsData {
        loan_token,
        collateral_token,
        oracle,
        irm,
        lltv,
    };
    println!("test_evm_end_to_end_morpho_not_reentrant, step 24 - MarketParamsData constructed: {:?}", market_params);

    if choice == 0 {
        // Testing test_SimpleSupplyWithdraw
        let supply_amount = U256::from_str("1000000000000000000000").unwrap();

        // Step 1: Set balance for supplier
        let operation = setBalanceCall { owner: address_supplier, amount: supply_amount };
        let operation = get_zero_operation(operation)?;
        node_service_regular.process_inbox(&chain2).await?;
        loan_token_regular.run_json_query(operation).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 25 - Set balance for supplier");

        // Step 2: Supplier supplies to Morpho
        let market_params_sol = MarketParams {
            loanToken: market_params.loan_token,
            collateralToken: market_params.collateral_token,
            oracle: market_params.oracle,
            irm: market_params.irm,
            lltv: market_params.lltv,
        };
        let operation = supplyCall {
            marketParams: market_params_sol.clone(),
            assets: supply_amount,
            shares: U256::ZERO,
            onBehalf: address_supplier,
            data: vec![].into(),
        };
        let operation = get_zero_operation(operation)?;
        node_service_supplier.process_inbox(&chain2).await?;
        morpho_supplier.run_json_query(operation).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 26 - Supplier supplied to Morpho");

        // Step 3: Check market state
        let query = idCall { };
        let query = EvmQuery::Query(query.abi_encode());
        let market_id_result = test_contract_regular.run_json_query(query).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 27 - Got market id: {:?}", market_id_result);

        // Parse the market_id from the result
        let market_id = parse_bytes32_from_array(&market_id_result)?;

        // Process inbox before querying market state
        node_service_regular.process_inbox(&chain2).await?;

        let query = marketCall { id: market_id.into() };
        let query = EvmQuery::Query(query.abi_encode());
        let market_state = morpho_regular.run_json_query(query).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 28 - Market state queried");

        // Parse and verify totalSupplyAssets == supplyAmount
        // Market state returns 6 uint128 values, each padded to 32 bytes
        let total_supply_assets = parse_u128_from_array_at_offset(&market_state, 0)?;
        let total_supply_assets_u256 = U256::from(total_supply_assets);

        // require(totalSupplyAssets == supplyAmount, "Total supply mismatch");
        assert_eq!(total_supply_assets_u256, supply_amount, "Total supply mismatch");
        println!("test_evm_end_to_end_morpho_not_reentrant, step 29 - Market state verified");

        // Step 4: Withdraw half
        let withdraw_amount = U256::from_str("500000000000000000000")?;
        let operation = withdrawCall {
            marketParams: market_params_sol.clone(),
            assets: withdraw_amount,
            shares: U256::ZERO,
            onBehalf: address_supplier,
            receiver: address_supplier,
        };
        let operation = get_zero_operation(operation)?;
        node_service_supplier.process_inbox(&chain2).await?;
        morpho_supplier.run_json_query(operation).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 30 - Withdrawal completed");

        // Step 5: Verify withdrawal by checking balance
        let query = balanceOfCall { owner: address_supplier };
        let query = EvmQuery::Query(query.abi_encode());
        let balance_result = loan_token_supplier.run_json_query(query).await?;
        println!("test_evm_end_to_end_morpho_not_reentrant, step 31 - Balance result: {:?}", balance_result);

        // Parse and verify balance
        let balance = parse_u256_from_array(&balance_result)?;
        assert_eq!(balance, withdraw_amount, "Withdrawal verification failed");
        println!("test_evm_end_to_end_morpho_not_reentrant, step 32 - Withdrawal verified successfully");
    }


    node_service_regular.ensure_is_running()?;
    node_service_owner.ensure_is_running()?;
    node_service_supplier.ensure_is_running()?;
    node_service_borrower.ensure_is_running()?;
    node_service_liquidator.ensure_is_running()?;
    node_service_supplier2.ensure_is_running()?;

    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Successful end");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Error: No test specified");
        eprintln!("Usage: {} <test-name>", args[0]);
        eprintln!("Available tests: morpho_not_reentrant, all");
        std::process::exit(1);
    }

    let test_name = &args[1];

    match test_name.as_str() {
        "morpho_not_reentrant" => {
            println!("Running EVM counter test...");
            test_evm_end_to_end_morpho_not_reentrant(0).await?;
        }
        _ => {
            eprintln!("Error: Unknown test '{}'", test_name);
            eprintln!("Available tests: evm-counter, all");
            std::process::exit(1);
        }
    }

    Ok(())
}
