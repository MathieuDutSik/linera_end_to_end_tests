use anyhow::Result;
use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
//use alloy_primitives::Address;
//use alloy_sol_types::SolValue;
use linera_base::vm::{EvmInstantiation, EvmOperation, EvmQuery};
use linera_sdk::{
//    abis::evm::EvmAbi,
    linera_base_types::{Account, Amount},
};
use std::{
//    collections::HashMap,
    path::PathBuf,
};

mod solidity;
use solidity::read_and_publish_contract;

use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use std::env;


fn get_zero_operation(operation: impl alloy_sol_types::SolCall) -> Result<EvmQuery, bcs::Error> {
    let operation = EvmOperation::new(Amount::ZERO, operation.abi_encode());
    operation.to_evm_query()
}

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
    config
}


async fn test_evm_end_to_end_morpho_not_reentrant() -> Result<()> {
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
        function test_ping() public pure returns (bool);
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

    // Test basic contract interaction first
    println!("test_evm_end_to_end_morpho_not_reentrant, step 10 - Testing basic contract interaction");
    let operation = test_pingCall { };
    let operation = EvmQuery::Query(operation.abi_encode());
    let result = test_contract_regular.run_json_query(operation).await?;
    println!("test_ping result: {:?}", result);

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

    // Step 2.5: Set oracle price (set_up_part_a2)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 15 - Running set_up_part_a2");
    let operation = set_up_part_a2Call { };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 16 - set_up_part_a2 completed");

    // Step 3: Enable IRM and LLTV
    println!("test_evm_end_to_end_morpho_not_reentrant, step 17 - Running set_up_part_b");
    let operation = set_up_part_bCall { };
    let operation = get_zero_operation(operation)?;
    node_service_owner.process_inbox(&chain2).await?;
    test_contract_owner.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 18 - set_up_part_b completed");

    // Step 4: Create market
    println!("test_evm_end_to_end_morpho_not_reentrant, step 19 - Running set_up_part_c");
    let operation = set_up_part_cCall { };
    let operation = get_zero_operation(operation)?;
    node_service_regular.process_inbox(&chain2).await?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 20 - set_up_part_c completed");

    // Step 5: Approve loan token (for all users)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 21 - Running set_up_part_d for all users");
    let operation = set_up_part_dCall { };
    let operation = get_zero_operation(operation)?;
    node_service_supplier.process_inbox(&chain2).await?;
    node_service_borrower.process_inbox(&chain2).await?;
    node_service_liquidator.process_inbox(&chain2).await?;
    node_service_supplier2.process_inbox(&chain2).await?;
    test_contract_supplier.run_json_query(operation.clone()).await?;
    test_contract_borrower.run_json_query(operation.clone()).await?;
    test_contract_liquidator.run_json_query(operation.clone()).await?;
    test_contract_supplier2.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - set_up_part_d completed");

    // Step 6: Approve collateral token (for borrower)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 23 - Running set_up_part_e");
    let operation = set_up_part_eCall { };
    let operation = get_zero_operation(operation)?;
    node_service_borrower.process_inbox(&chain2).await?;
    test_contract_borrower.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 24 - set_up_part_e completed");

/*
    let operation = test_SimpleSupplyWithdrawCall { };
    let operation = get_zero_operation(operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 12 - Running test_SimpleSupplyWithdraw");
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 13 - test_SimpleSupplyWithdraw completed");
*/
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
            test_evm_end_to_end_morpho_not_reentrant().await?;
        }
        _ => {
            eprintln!("Error: Unknown test '{}'", test_name);
            eprintln!("Available tests: evm-counter, all");
            std::process::exit(1);
        }
    }

    Ok(())
}
