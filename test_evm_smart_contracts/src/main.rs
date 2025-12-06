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

    // Deploy ERC20Mock for loanToken
    println!("test_evm_end_to_end_morpho_not_reentrant, step 3 - Deploying loanToken (ERC20Mock)");
    let loan_token_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "src/mocks/ERC20Mock.sol",
        "ERC20Mock",
        constructor_argument.clone(),
        evm_instantiation.clone()
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, loan_token_app_id={:?}", loan_token_app_id);

    // Deploy ERC20Mock for collateralToken
    println!("test_evm_end_to_end_morpho_not_reentrant, step 4 - Deploying collateralToken (ERC20Mock)");
    let collateral_token_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "src/mocks/ERC20Mock.sol",
        "ERC20Mock",
        constructor_argument.clone(),
        evm_instantiation.clone()
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, collateral_token_app_id={:?}", collateral_token_app_id);

    // Deploy OracleMock
    println!("test_evm_end_to_end_morpho_not_reentrant, step 5 - Deploying oracle (OracleMock)");
    let oracle_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "src/mocks/OracleMock.sol",
        "OracleMock",
        constructor_argument.clone(),
        evm_instantiation.clone()
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, oracle_app_id={:?}", oracle_app_id);

    // Deploy IrmMock
    println!("test_evm_end_to_end_morpho_not_reentrant, step 6 - Deploying IRM (IrmMock)");
    let irm_app_id = read_and_publish_contract(
        &client_regular,
        &path,
        "src/mocks/IrmMock.sol",
        "IrmMock",
        constructor_argument.clone(),
        evm_instantiation.clone()
    ).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, irm_app_id={:?}", irm_app_id);

    // Create EVM addresses for all deployed contracts
    println!("test_evm_end_to_end_morpho_not_reentrant, step 6.5 - Creating EVM addresses");
    use alloy_primitives::Address;

    let loan_token_address = loan_token_app_id.evm_address();
    let collateral_token_address = collateral_token_app_id.evm_address();
    let oracle_address = oracle_app_id.evm_address();
    let irm_address = irm_app_id.evm_address();

    println!("loan_token_address: {:?}", loan_token_address);
    println!("collateral_token_address: {:?}", collateral_token_address);
    println!("oracle_address: {:?}", oracle_address);
    println!("irm_address: {:?}", irm_address);

    // Deploy Morpho with owner parameter
    println!("test_evm_end_to_end_morpho_not_reentrant, step 6.6 - Deploying Morpho");

    // Extract the owner address from owner_owner
    let owner_address = owner_owner.to_evm_address().unwrap();
    println!("owner_address: {:?}", owner_address);

    let morpho_constructor = MorphoConstructor {
        newOwner: owner_address,
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
    let loan_token_app = node_service_regular.make_application(&chain2, &loan_token_app_id)?;
    let collateral_token_app = node_service_regular.make_application(&chain2, &collateral_token_app_id)?;
    let oracle_app = node_service_regular.make_application(&chain2, &oracle_app_id)?;
    let irm_app = node_service_regular.make_application(&chain2, &irm_app_id)?;
    let morpho_app = node_service_regular.make_application(&chain2, &morpho_app_id)?;
    let test_contract_app = node_service_regular.make_application(&chain2, &test_contract_app_id)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 9 - All application wrappers created");

    // Setup oracle price (1:1 ratio)
    // ORACLE_PRICE_SCALE = 1e36 (from SimpleNonReentrantTest.sol line 40)
    use alloy_primitives::U256;
    let oracle_price_scale = U256::from(10).pow(U256::from(36));
    let set_price_operation = setPriceCall { newPrice: oracle_price_scale };
    let set_price_operation = get_zero_operation(set_price_operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 9.5 - Setting oracle price to {:?}", oracle_price_scale);
    oracle_app.run_json_query(set_price_operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 9.6 - Oracle price set");

    let operation = setUpCall { };
    let operation = get_zero_operation(operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 10 - Running setUp");
    println!("test_evm_end_to_end_morpho_not_reentrant, operation={:?}", operation);
    test_contract_app.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 11 - setUp completed");

/*
    let operation = test_SimpleSupplyWithdrawCall { };
    let operation = get_zero_operation(operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 12 - Running test_SimpleSupplyWithdraw");
    test_contract_app.run_json_query(operation).await?;
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
