use anyhow::Result;
use alloy_sol_types::sol;
//use alloy_sol_types::SolCall;
use alloy_sol_types::SolValue;
use linera_base::vm::{EvmInstantiation, EvmOperation, EvmQuery};
use linera_sdk::{
//    abis::evm::EvmAbi,
    linera_base_types::Amount,
};
use std::{
    collections::HashMap,
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
            Amount::from_tokens(12),
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

    sol! {
        function setUp();
        function test_SimpleSupplyWithdraw();
        function setPrice(uint256 newPrice);
        function deploy_mocks();
        function set_addresses(
            address ownerAddress,
            address supplierAddress,
            address borrowerAddress,
            address liquidatorAddress,
            address supplier2Address
        );
        function set_up_part_a();
        function set_up_part_b();
        function set_up_part_c();
        function set_up_part_d();
        function set_up_part_e();

        // Morpho constructor
        struct MorphoConstructor {
            address newOwner;
        }
    }

    println!("test_evm_end_to_end_morpho_not_reentrant, step 1 - Deploying contracts");
    let path = PathBuf::from("morpho_test_code/result.out");
    println!("test_evm_end_to_end_morpho_not_reentrant, step 2");

    let constructor_argument = Vec::new();
    let evm_instantiation = EvmInstantiation::default();

    // Deploy Morpho with owner parameter
    println!("test_evm_end_to_end_morpho_not_reentrant, step 3 - Deploying Morpho");
    use alloy_primitives::Address;

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

    let morpho_constructor = MorphoConstructor {
        newOwner: address_owner,
    };
    use alloy_sol_types::SolConstructor;
    let morpho_constructor_args = morpho_constructor.abi_encode();
    let morpho_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "src/Morpho.sol",
        "Morpho",
        morpho_constructor_args,
        evm_instantiation.clone()
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, morpho_app_id={:?}", morpho_app_id);

    let morpho_address = morpho_app_id.evm_address();
    println!("morpho_address: {:?}", morpho_address);

    // Deploy SimpleNonReentrantTest
    println!("test_evm_end_to_end_morpho_not_reentrant, step 7 - Deploying test contract (SimpleNonReentrantTest)");
    let test_contract_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "SimpleNonReentrantTest.sol",
        "SimpleNonReentrantTest",
        constructor_argument.clone(),
        evm_instantiation
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

    // Create application wrappers for all deployed contracts
    let morpho_app = node_service_regular.make_application(&chain2, &morpho_app_id)?;

    // Create test contract application wrappers for each user
    let test_contract_regular = node_service_regular.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_owner = node_service_owner.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_supplier = node_service_supplier.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_borrower = node_service_borrower.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_liquidator = node_service_liquidator.make_application(&chain2, &test_contract_app_id)?;
    let test_contract_supplier2 = node_service_supplier2.make_application(&chain2, &test_contract_app_id)?;

    println!("test_evm_end_to_end_morpho_not_reentrant, step 9 - All application wrappers created");

    // Step 1: Deploy mock contracts
    println!("test_evm_end_to_end_morpho_not_reentrant, step 10 - Deploying mocks");
    let operation = deploy_mocksCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 11 - Mocks deployed");

    // Step 2: Set user addresses
    println!("test_evm_end_to_end_morpho_not_reentrant, step 12 - Setting user addresses");
    let operation = set_addressesCall {
        ownerAddress: address_owner,
        supplierAddress: address_supplier,
        borrowerAddress: address_borrower,
        liquidatorAddress: address_liquidator,
        supplier2Address: address_supplier2,
    };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 13 - User addresses set");

    // Step 3: Verify setup (set_up_part_a)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 14 - Running set_up_part_a");
    let operation = set_up_part_aCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_owner.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 15 - set_up_part_a completed");

    // Step 4: Enable IRM and LLTV
    println!("test_evm_end_to_end_morpho_not_reentrant, step 16 - Running set_up_part_b");
    let operation = set_up_part_bCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_owner.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 17 - set_up_part_b completed");

    // Step 5: Create market
    println!("test_evm_end_to_end_morpho_not_reentrant, step 18 - Running set_up_part_c");
    let operation = set_up_part_cCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_regular.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 19 - set_up_part_c completed");

    // Step 6: Approve loan token (for all users)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 20 - Running set_up_part_d for all users");
    let operation = set_up_part_dCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_supplier.run_json_query(operation.clone()).await?;
    test_contract_borrower.run_json_query(operation.clone()).await?;
    test_contract_liquidator.run_json_query(operation.clone()).await?;
    test_contract_supplier2.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 21 - set_up_part_d completed");

    // Step 7: Approve collateral token (for borrower)
    println!("test_evm_end_to_end_morpho_not_reentrant, step 22 - Running set_up_part_e");
    let operation = set_up_part_eCall { };
    let operation = get_zero_operation(operation)?;
    test_contract_borrower.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 23 - set_up_part_e completed");

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
