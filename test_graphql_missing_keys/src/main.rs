use anyhow::Result;
use linera_base::vm::VmRuntime;
use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use linera_service::cli_wrappers::ClientWrapper;
use std::path::PathBuf;
//use linera_base::async_graphql::InputType;
//use linera_base::async_graphql::ScalarType;
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

async fn end_to_end_complex_data() -> Result<()> {
    use complex_data_contract::ComplexDataAbi;
    let config = get_config();

    let (mut net, client) = config.instantiate().await?;
    println!("end_to_end_complex_data, step 1");

    let chain_id = client.load_wallet()?.default_chain().unwrap();
    println!("end_to_end_complex_data, step 2");

    let (contract_path, service_path) = build_application(&client, "complex-data-contract").await?;
    println!("end_to_end_complex_data, step 3");

    let application_id = client
        .publish_and_create::<ComplexDataAbi, (), ()>(
            contract_path,
            service_path,
            VmRuntime::Wasm,
            &(),
            &(),
            &[],
            None,
        )
        .await?;
    println!("end_to_end_complex_data, step 4");

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let app_id = node_service.make_application(&chain_id, &application_id)?;
    println!("end_to_end_complex_data, step 5");

    // Field1
    // SET

    let value_set = 12;
    let mutation = format!(
        "setField1(value: {})",
        value_set,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 6");

    // READ

    let query = "field1";
    let value_read: u64 = app_id.query_json(&query).await.unwrap();
    assert_eq!(value_read, value_set);
    println!("end_to_end_complex_data, step 7");


    // Field2
    // SET

    let key1 = "Bonjour";
    let value1 = 49;
    let mutation = format!(
        "insertField2(key: \"{}\", value: {})",
        key1, value1,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 8");





    // Field2



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
        eprintln!("Available tests: repeated-fungible, repeated-fungible-no-graphql, all");
        std::process::exit(1);
    }

    let test_name = &args[1];

    match test_name.as_str() {
        "complex-data" => {
            println!("Running repeated-fungibl test...");
            end_to_end_complex_data().await?;
        }
        _ => {
            eprintln!("Error: Unknown test '{}'", test_name);
            eprintln!("Available tests: create-and-call, blob-access, all");
            std::process::exit(1);
        }
    }

    Ok(())
}
