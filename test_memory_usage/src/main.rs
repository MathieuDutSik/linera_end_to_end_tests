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

fn get_critical_pids() -> Vec<(String, u16)> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut pids = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        if name == "linera-server" || name == "linera-proxy" {
            pids.push(pid);
        }
    }
    pids
}

fn print_memory_usages(pids: &[(String,u16)]) {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    for (name, pid) in pids {
        if let Some(p) = sys.process(pid) {
            // memory() = RSS; virtual_memory() = virtual size (a.k.a. VMS)
            println!("name={name} pid={pid} RSS bytes: {}, Virtual bytes: {}", p.memory(), p.virtual_memory());
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    use state_triviality::{StateTrivialityAbi, StateTrivialityRequest};

    let config = get_config();

    tracing::info!("Starting state triviality end-to-end test");
    let (mut net, client) = config.instantiate().await?;


    let critical_pids = get_critical_pids();



    // Step 2: Instantiate the contract "state-triviality"
    println!("Step 2: Publishing and creating state-triviality application");
    let chain = client.load_wallet()?.default_chain().unwrap();
    let name2 = "counter-no-state";
    let path2 = env::current_dir()?.join("./smart_contract_code/").join(name2);
    let (counter_contract, counter_service) =
        client.build_application(&path2, name2, true).await?;
    let application_id = client
        .publish_and_create::<StateTrivialityAbi, (), ()>(
            counter_contract,
            counter_service,
            VmRuntime::Wasm,
            &(),
            &(), // Initial value: nothing
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
