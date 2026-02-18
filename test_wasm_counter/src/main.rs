use anyhow::Result;
use linera_base::vm::VmRuntime;
use linera_service::cli_wrappers::{
    local_net::{get_node_port, Database, LocalNetConfig, ProcessInbox},
    LineraNet, LineraNetConfig, Network,
};
use linera_sdk::linera_base_types::Account;
use counter::CounterAbi;

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
    let path = "/Users/mathieudutoursikiric/GITlineraOpen/GITout_of_scope_end_to_end_tests/linera-protocol_second/target/debug";
    let path = std::path::PathBuf::from(path);
    config.binary_dir = Some(path);
    config
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("main, step 1");
    let config = get_config();
    println!("main, step 2");

    let (mut net, client) = config.instantiate().await?;
    println!("main, step 3");

    let original_counter_value = 35;
    let increment = 5;

    let chain = client.load_wallet()?.default_chain().unwrap();
    let account_chain = Account::chain(chain);
    println!("main, step 4");

    let name1 = "counter";
    let path1 = std::env::current_dir()?.join("./smart_contract_code/").join(name1);
    let (contract_path, service_path) =
        client.build_application(&path1, name1, true).await?;
    println!("main, step 5");

    let application_id = client
        .publish_and_create::<CounterAbi, (), u64>(
            contract_path,
            service_path,
            VmRuntime::Wasm,
            &(),
            &original_counter_value,
            &[],
            None,
        )
        .await?;
    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;

    let application = node_service.make_application(&chain, &application_id)?;

    let balance1 = node_service.balance(&account_chain).await?;

    let counter_value: u64 = application.query_json("value").await?;
    assert_eq!(counter_value, original_counter_value);
    let balance2 = node_service.balance(&account_chain).await?;
    assert_eq!(balance1, balance2);

    let mutation = format!("increment(value: {increment})");
    application.mutate(mutation).await?;
    let balance3 = node_service.balance(&account_chain).await?;
    assert!(balance3 < balance2);

    let counter_value: u64 = application.query_json("value").await?;
    assert_eq!(counter_value, original_counter_value + increment);

    node_service.ensure_is_running()?;

    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Normal termination of the program");
    Ok(())
}
