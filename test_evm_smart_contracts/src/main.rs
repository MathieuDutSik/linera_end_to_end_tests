use anyhow::Result;
use alloy_sol_types::sol;
//use alloy_sol_types::{SolCall, SolValue};
use linera_base::vm::{EvmOperation, EvmQuery};
use linera_sdk::{
//    abis::evm::EvmAbi,
    linera_base_types::Amount,
};
use std::{
    collections::HashMap,
    path::PathBuf,
};

mod solidity;
use solidity::read_and_publish_contracts;

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
    // Creating the clients
    let (mut net, client) = config.instantiate().await?;

    sol! {
        function setUp();
        function test_SimpleSupplyWithdraw();
    }

    // Building the chain
    let chain = *client.load_wallet()?.chain_ids().first().unwrap();

    println!("test_evm_end_to_end_morpho_not_reentrant, step 1");
    let path = PathBuf::from("morpho_test_code/result.out");
    println!("test_evm_end_to_end_morpho_not_reentrant, step 2");

    let map = HashMap::new();
    let application_id = read_and_publish_contracts(&client, &path, "SimpleNonReentrantTest.sol", "SimpleNonReentrantTest", &map).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 5");
    println!("test_evm_end_to_end_morpho_not_reentrant, application_id={:?}", application_id);

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 6");


    let application = node_service.make_application(&chain, &application_id)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 7");

    let operation = setUpCall { };
    let operation = get_zero_operation(operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 8");
    println!("test_evm_end_to_end_morpho_not_reentrant, operation={:?}", operation);
    application.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 9");

/*
    let operation = test_SimpleSupplyWithdrawCall { };
    let operation = get_zero_operation(operation)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 10");
    application.run_json_query(operation).await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 11");
*/
    node_service.ensure_is_running()?;

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
