mod specified_local_net;
use specified_local_net::{Database, SpecifiedLocalNetConfig};

use anyhow::Result;
use linera_base::{
    data_types::Amount,
    identifiers::{Account, AccountOwner},
    vm::VmRuntime,
};

use linera_service::cli_wrappers::{
    local_net::{get_node_port, ProcessInbox},
    LineraNet, LineraNetConfig, Network,
};
use linera_service::cli_wrappers::{ClientWrapper, NotificationsExt};
use std::path::PathBuf;
use std::env;

fn get_config() -> SpecifiedLocalNetConfig {
    let mut config = SpecifiedLocalNetConfig::new_test(Database::ScyllaDb, Network::Grpc);
    config.num_initial_validators = 4;
    config.num_shards = 4;
    config
}

async fn build_application(client: &ClientWrapper, name: &str) -> Result<(PathBuf, PathBuf)> {
    let path = env::current_dir()?.join("./smart_contract_code/").join(name);
    Ok(client.build_application(&path, name, true).await?)
}



async fn test_wasm_end_to_end_social_event_streams() -> Result<()> {
    use social::SocialAbi;

    let config = get_config();
    let (mut net, client1) = config.instantiate().await?;

    let client2 = net.make_client().await;
    client2.wallet_init(None).await?;

    // We use a newly opened chain for the publisher, so that client2 will not be listening to that
    // chain by default.
    let chain1 = client1
        .open_and_assign(&client1, Amount::from_tokens(100))
        .await?;
    let chain2 = client1.open_and_assign(&client2, Amount::ONE).await?;
    let (contract, service) = build_application(&client1, "social").await?;
    let module_id = client1
        .publish_module::<SocialAbi, (), ()>(contract, service, VmRuntime::Wasm, None)
        .await?;
    let application_id = client1
        .create_application(&module_id, &(), &(), &[], None)
        .await?;

    let port1 = get_node_port().await;
    let port2 = get_node_port().await;
    let mut node_service1 = client1
        .run_node_service(port1, ProcessInbox::Automatic)
        .await?;
    let mut node_service2 = client2
        .run_node_service(port2, ProcessInbox::Automatic)
        .await?;

    let app2 = node_service2.make_application(&chain2, &application_id)?;
    app2.mutate(format!("subscribe(chainId: \"{chain1}\")"))
        .await?;
    let (_, height2) = node_service2.chain_tip(chain2).await?.unwrap();

    let mut notifications = node_service2.notifications(chain2).await?;

    let app1 = node_service1.make_application(&chain1, &application_id)?;
    app1.mutate("post(text: \"Linera Social is the new Mastodon!\")")
        .await?;

    let query = "receivedPosts { keys { author, index } }";
    let expected_response = serde_json::json!({
        "receivedPosts": {
            "keys": [
                { "author": chain1, "index": 0 }
            ]
        }
    });
    notifications.wait_for_block(height2.try_add_one()?).await?;
    assert_eq!(app2.query(query).await?, expected_response);

    let tip_after_first_post = node_service2.chain_tip(chain1).await?;

    // Perform an operation that does not emit events, or messages that client 2 listens to - to be
    // safe, we just transfer from chain1 to itself.
    node_service1
        .transfer(
            chain1,
            AccountOwner::CHAIN,
            Account::chain(chain1),
            Amount::ONE,
        )
        .await?;

    let (_, height2) = node_service2.chain_tip(chain2).await?.unwrap();
    app1.mutate("post(text: \"Second post!\")").await?;

    let query = "receivedPosts { keys { author, index } }";
    let expected_response = serde_json::json!({
        "receivedPosts": {
            "keys": [
                { "author": chain1, "index": 1 },
                { "author": chain1, "index": 0 }
            ]
        }
    });
    notifications.wait_for_block(height2.try_add_one()?).await?;
    assert_eq!(app2.query(query).await?, expected_response);

    let tip_after_second_post = node_service2.chain_tip(chain1).await?;
    // The second post should not have moved the tip hash - client 2 should have only preprocessed
    // that block, without downloading the transfer block in between.
    assert_eq!(tip_after_first_post, tip_after_second_post);

    node_service1.ensure_is_running()?;
    node_service2.ensure_is_running()?;

    net.ensure_is_running().await?;
    net.terminate().await?;

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
        "social" => {
            println!("Running social test...");
            test_wasm_end_to_end_social_event_streams().await?;
        }
        _ => {
            eprintln!("Error: Unknown test '{}'", test_name);
            std::process::exit(1);
        }
    }

    Ok(())
}
