use anyhow::Result;
use linera_base::vm::VmRuntime;
use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use linera_service::cli_wrappers::ClientWrapper;
use std::path::PathBuf;
use linera_sdk::serde_json;
use std::env;

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
    config
}

async fn build_application(client: &ClientWrapper, name: &str) -> Result<(PathBuf, PathBuf)> {
    let path = env::current_dir()?.join("./smart_contract_code/").join(name);
    Ok(client.build_application(&path, name, true).await?)
}

#[tokio::main]
async fn main() -> Result<()> {
    use reported_solutions::ReportedSolutionsAbi;
    let config = get_config();

    let (mut net, client) = config.instantiate().await?;

    let chain_id = client.load_wallet()?.default_chain().unwrap();

    let (contract_path, service_path) = build_application(&client, "reported-solutions").await?;

    let application_id = client
        .publish_and_create::<ReportedSolutionsAbi, (), ()>(
            contract_path,
            service_path,
            VmRuntime::Wasm,
            &(),
            &(),
            &[],
            None,
        )
        .await?;

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let app_id = node_service.make_application(&chain_id, &application_id)?;

    let query = "reportedSolutions { entries { key, value { count } } }";
    let response_body = app_id.query(&query).await.unwrap();
    println!("end_to_end_complex_data, step 25, response_body={}", response_body);


    node_service.ensure_is_running()?;
    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Successful end");
    Ok(())
}
