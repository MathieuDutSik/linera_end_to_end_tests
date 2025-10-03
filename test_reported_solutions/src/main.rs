use anyhow::Result;
use linera_base::vm::VmRuntime;
use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use linera_service::cli_wrappers::ClientWrapper;
use std::path::PathBuf;
use linera_sdk::serde_json;
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
    println!("end_to_end_complex_data, step 01");

    let chain_id = client.load_wallet()?.default_chain().unwrap();
    println!("end_to_end_complex_data, step 02");

    let (contract_path, service_path) = build_application(&client, "complex-data-contract").await?;
    println!("end_to_end_complex_data, step 03");

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
    println!("end_to_end_complex_data, step 04");

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let app_id = node_service.make_application(&chain_id, &application_id)?;
    println!("end_to_end_complex_data, step 05");

    // Field1
    // SET

    let value_set = 12;
    let mutation = format!(
        "setField1(value: {})",
        value_set,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 06");

    // READ

    let query = "field1";
    let value_read: u64 = app_id.query_json(&query).await.unwrap();
    assert_eq!(value_read, value_set);
    println!("end_to_end_complex_data, step 07");


    // Prefield2
    // SET

    let key1 = "Bonjour";
    let value1 = 49;
    let mutation = format!(
        "insertPrefield2(key: \"{}\", value: {})",
        key1, value1,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 08");

    // READ1

    let query = "prefield2 { keys }";
    let response_body = app_id.query(&query).await.unwrap();
    println!("end_to_end_complex_data, step 09, response_body={}", response_body);
    let keys: Vec<String> = serde_json::from_value(response_body["prefield2"]["keys"].clone()).unwrap();
    println!("end_to_end_complex_data, step 10, keys={:?}", keys);
    assert_eq!(keys, vec!["Bonjour".to_string()]);
    println!("end_to_end_complex_data, step 11");

    // READ2

    let query = "prefield2 { entries { key, value } }";
    println!("end_to_end_complex_data, step 12, query={}", query);
    let response_body = app_id.query(&query).await.unwrap();
    println!("end_to_end_complex_data, step 13, response_body={}", response_body);



    // Field2
    // SET

    let key1 = "Bonjour";
    let value1 = 49;
    let mutation = format!(
        "insertField2(key: \"{}\", value: {})",
        key1, value1,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 14");

    // READ1

    let query = "field2 { keys }";
    let response_body = app_id.query(&query).await.unwrap();
    println!("end_to_end_complex_data, step 15, response_body={}", response_body);
    let keys: Vec<String> = serde_json::from_value(response_body["field2"]["keys"].clone()).unwrap();
    println!("end_to_end_complex_data, step 16, keys={:?}", keys);
    assert_eq!(keys, vec!["Bonjour".to_string()]);
    println!("end_to_end_complex_data, step 17");

    // READ2

    if false {
        // This is failing because the value is a value and so it cannot
        let _failing_query = "field2 { entries { key, value } }";
        let query = "field2 { entries { key, value { value } } }";
        println!("end_to_end_complex_data, step 18, query={}", query);
        let response_body = app_id.query(&query).await.unwrap();
        println!("end_to_end_complex_data, step 19, response_body={}", response_body);
    }




    // Field4
    // SET

    let key1 = "Bonjour";
    let key2 = "Hello";
    let value1 = 124;
    let mutation = format!(
        "insertField4(key1: \"{}\", key2: \"{}\", value: {})",
        key1, key2, value1,
    );
    app_id.mutate(&mutation).await?;
    println!("end_to_end_complex_data, step 20");

    // READ1

    let query = "field4 { keys }";
    let response_body = app_id.query(&query).await.unwrap();
    println!("end_to_end_complex_data, step 21, response_body={}", response_body);
    let keys: Vec<String> = serde_json::from_value(response_body["field4"]["keys"].clone()).unwrap();
    println!("end_to_end_complex_data, step 22, keys={:?}", keys);
    assert_eq!(keys, vec!["Bonjour".to_string()]);
    println!("end_to_end_complex_data, step 23");

    // READ2

    if true {
        let query = "field4 { entries { key, value { count } } }";
        println!("end_to_end_complex_data, step 24, query={}", query);
        let response_body = app_id.query(&query).await.unwrap();
        println!("end_to_end_complex_data, step 25, response_body={}", response_body);
    }




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
