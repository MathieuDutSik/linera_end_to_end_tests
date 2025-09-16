use anyhow::Result;
use linera_base::{data_types::Bytecode, vm::VmRuntime};
use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use std::env;

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
    config
}

#[tokio::main]
async fn main() -> Result<()> {
    use state_triviality::{StateTrivialityAbi, StateTrivialityRequest};

    let config = get_config();

    tracing::info!("Starting state triviality end-to-end test");
    let (mut net, client) = config.instantiate().await?;

    // Step 1: Download the contract and service of "counter-no-graphql" as Vec<u8>
    println!("Step 1: Building counter-no-state example");
    let name1 = "counter-no-state";
    let path1 = env::current_dir()?.join("./smart_contract_code/").join(name1);
    let (counter_contract_path, counter_service_path) =
        client.build_application(&path1, name1, true).await?;
    let counter_contract_bytecode = Bytecode::load_from_file(&counter_contract_path).await?;
    let counter_service_bytecode = Bytecode::load_from_file(&counter_service_path).await?;
    let contract_bytes = counter_contract_bytecode.bytes;
    let service_bytes = counter_service_bytecode.bytes;

    // Step 2: Instantiate the contract "state-triviality"
    println!("Step 2: Publishing and creating state-triviality application");
    let chain = client.load_wallet()?.default_chain().unwrap();
    let name2 = "state-triviality";
    let path2 = env::current_dir()?.join("./smart_contract_code/").join(name2);
    let (create_call_contract, create_call_service) =
        client.build_application(&path2, name2, true).await?;
    let application_id = client
        .publish_and_create::<StateTrivialityAbi, (), ()>(
            create_call_contract,
            create_call_service,
            VmRuntime::Wasm,
            &(),
            &(), // Initial value
            &[],
            None,
        )
        .await?;

    println!("Step 3: Starting node service and creating application wrapper");
    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let application = node_service
        .make_application(&chain, &application_id)
        .await?;

    // Step 4: Call a mutation that takes the Vec<u8> of "contract", "service",
    println!("Step 4: Calling CreateAndCall mutation with increment_value=5");
    let increment_value = 5;
    let mutation_request = StateTrivialityRequest::CreateAndCall(
        contract_bytes,
        service_bytes,
        increment_value,
    );
    application.run_json_query(&mutation_request).await?;

    println!("Test completed successfully!");

    node_service.ensure_is_running()?;
    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Successful end");
    Ok(())
}
