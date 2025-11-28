use anyhow::Result;
use alloy_sol_types::{sol, SolCall, SolValue};
use linera_base::vm::{VmRuntime, EvmInstantiation, EvmOperation, EvmQuery};
use linera_sdk::{
    abis::evm::EvmAbi,
    linera_base_types::Amount,
};
use std::path::PathBuf;

mod solidity;
use solidity::{get_evm_contract_path, temporary_write_evm_module, read_evm_u64_entry, read_bytecode_from_file};

use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use std::env;



// Linera Solidity library constants
const LINERA_SOL: &str = include_str!("../solidity/Linera.sol");
const LINERA_TYPES_SOL: &str = include_str!("../solidity/LineraTypes.sol");

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
//    let path = PathBuf::from("/Users/mathieudutoursikiric/GITall/GITmathieu/linera_end_to_end_tests/test_evm_smart_contracts/morpho_test_code/result.out");
    let path = PathBuf::from("morpho_test_code/result.out");
    println!("test_evm_end_to_end_morpho_not_reentrant, step 2");
    let module = read_bytecode_from_file(&path, "SimpleNonReentrantTest.sol", "SimpleNonReentrantTest")?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 3 |module|={}", module.len());
    let (evm_contract, _dir) = temporary_write_evm_module(module)?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 4");

    let constructor_argument = Vec::new();
    let instantiation_argument = EvmInstantiation::default();
    let application_id = client
        .publish_and_create::<EvmAbi, Vec<u8>, EvmInstantiation>(
            evm_contract.clone(),
            evm_contract,
            VmRuntime::Evm,
            &constructor_argument,
            &instantiation_argument,
            &[],
            None,
        )
        .await?;
    println!("test_evm_end_to_end_morpho_not_reentrant, step 5");

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    println!("test_evm_end_to_end_divvi, step 3");

    let application = node_service.make_application(&chain, &application_id)?;

    let operation = setUpCall { };
    let operation = get_zero_operation(operation)?;
    application.run_json_query(operation).await?;


    let operation = test_SimpleSupplyWithdrawCall { };
    let operation = get_zero_operation(operation)?;
    application.run_json_query(operation).await?;

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
