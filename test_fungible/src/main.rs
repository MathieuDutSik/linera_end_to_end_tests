use anyhow::Result;
use linera_base::{
    data_types::Amount,
    identifiers::Account,
    vm::VmRuntime,
    time::Instant,
};
use linera_service::cli_wrappers::{
    local_net::{get_node_port, LocalNetConfig, ProcessInbox, Database},
    LineraNet, LineraNetConfig, Network,
};
use linera_service::cli_wrappers::ClientWrapper;
use std::path::PathBuf;
use linera_base::async_graphql::InputType;
//use linera_base::async_graphql::ScalarType;
use std::{collections::BTreeMap, env};

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 4;
    config.num_shards = 4;
    config
}

async fn build_application(client: &ClientWrapper, name: &str) -> Result<(PathBuf, PathBuf)> {
    let path = env::current_dir()?.join("./smart_contract_code/").join(name);
    Ok(client.build_application(&path, name, true).await?)
}

async fn end_to_end_repeated_transfer_fungible() -> Result<()> {
    let num_operations = 500;
    use fungible::{FungibleTokenAbi, InitialState, Parameters};
    let config = get_config();

    tracing::info!("Starting repeated transfer in fungible");
    let (mut net, client) = config.instantiate().await?;

    let chain_id = client.load_wallet()?.default_chain().unwrap();

    let account_owner1 = client.get_owner().unwrap();
    let account_owner2 = client.keygen().await?;


    let (contract_path, service_path) = build_application(&client, "fungible").await?;


    let params = Parameters::new("NAT");
    let accounts = BTreeMap::from([
        (account_owner1, Amount::from_tokens(1000)),
    ]);
    let state = InitialState { accounts };
    let application_id = client
        .publish_and_create::<FungibleTokenAbi, Parameters, InitialState>(
            contract_path,
            service_path,
            VmRuntime::Wasm,
            &params,
            &state,
            &[],
            None,
        )
        .await?;

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let app_id = node_service.make_application(&chain_id, &application_id)?;

    let amount_transfer = Amount::ONE;
    let destination = Account {
        chain_id,
        owner: account_owner2,
    };
    let mutation = format!(
        "transfer(owner: {}, amount: \"{}\", targetAccount: {})",
        account_owner1.to_value(),
        amount_transfer,
        destination.to_value(),
    );
    let mutations = vec![mutation; num_operations];
    let time_start = Instant::now();
    app_id.multiple_mutate(&mutations).await?;
    let average_time = (time_start.elapsed().as_millis() as f64) / (num_operations as f64);
    println!("Average runtime for fungible transfer={average_time}");

    node_service.ensure_is_running()?;
    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Successful end");
    Ok(())
}


async fn end_to_end_repeated_transfer_fungible_no_graphql() -> Result<()> {
    let num_operations = 500;
    use fungible_no_graphql::{FungibleNoGraphQlTokenAbi, FungibleOperation, FungibleNoGraphQlRequest};
    use fungible::{InitialState, Parameters};
    let config = get_config();

    tracing::info!("Starting repeated transfer in fungible");
    let (mut net, client) = config.instantiate().await?;

    let chain_id = client.load_wallet()?.default_chain().unwrap();

    let account_owner1 = client.get_owner().unwrap();
    let account_owner2 = client.keygen().await?;


    let (contract_path, service_path) = build_application(&client, "fungible-no-graphql").await?;


    let params = Parameters::new("NAT");
    let accounts = BTreeMap::from([
        (account_owner1, Amount::from_tokens(1000)),
    ]);
    let state = InitialState { accounts };
    let application_id = client
        .publish_and_create::<FungibleNoGraphQlTokenAbi, Parameters, InitialState>(
            contract_path,
            service_path,
            VmRuntime::Wasm,
            &params,
            &state,
            &[],
            None,
        )
        .await?;

    let port = get_node_port().await;
    let mut node_service = client.run_node_service(port, ProcessInbox::Skip).await?;
    let app_id = node_service.make_application(&chain_id, &application_id)?;

    let amount = Amount::ONE;
    let target_account = Account {
        chain_id,
        owner: account_owner2,
    };
    let mut operations = Vec::new();
    for _ in 0..num_operations {
        operations.push(FungibleOperation::Transfer {
            owner: account_owner1,
            amount,
            target_account,
        });
    }
    let query = FungibleNoGraphQlRequest::Operations { operations };

    let time_start = Instant::now();
    app_id.run_json_query(&query).await?;
    let average_time = (time_start.elapsed().as_millis() as f64) / (num_operations as f64);
    println!("Average runtime for fungible-no-graphql transfer={average_time}");

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
        "repeated-fungible" => {
            println!("Running repeated-fungible test...");
            end_to_end_repeated_transfer_fungible().await?;
        }
        "repeated-fungible-no-graphql" => {
            println!("Running repeated-fungible-no-graphql test...");
            end_to_end_repeated_transfer_fungible_no_graphql().await?;
        }
        "all" => {
            println!("Running repeated-fungible / repeated-fungible-no-graphql test...");
            end_to_end_repeated_transfer_fungible().await?;
            end_to_end_repeated_transfer_fungible_no_graphql().await?;
        }
        _ => {
            eprintln!("Error: Unknown test '{}'", test_name);
            eprintln!("Available tests: create-and-call, blob-access, all");
            std::process::exit(1);
        }
    }

    Ok(())
}
