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

fn get_directory_old_schema() -> String {
    let directory = std::env::current_dir();
    directory.extend("linera-protocol_test_conway_old_schema/target/debug");
}

fn get_directory_new_schema() -> String {
    let directory = std::env::current_dir();
    directory.extend("linera-protocol_test_conway_new_schema/target/debug");
}

fn get_config() -> SpecifiedLocalNetConfig {
    let directory = get_directory_old_schema();
    let mut config = SpecifiedLocalNetConfig::new_test(Database::ScyllaDb, Network::Grpc, directory);
    config.num_initial_validators = 4;
    config.num_shards = 1;
    config
}

async fn build_application(client: &ClientWrapper, name: &str) -> Result<(PathBuf, PathBuf)> {
    let path = env::current_dir()?.join("./smart_contract_code/").join(name);
    Ok(client.build_application(&path, name, true).await?)
}



fn value_to_btreemap(value: &Value) -> Option<BTreeMap<String, String>> {
    // Ensure it's a JSON object
    let obj = value.as_object()?;
    let mut map = BTreeMap::new();
    for (k, v) in obj {
        // Convert each value to a string representation
        let s = match v {
            Value::String(s) => s.clone(),
            _ => return None;
        };
        map.insert(k.clone(), s);
    }
    Some(map)
}

fn random_post() -> String {
    let charset: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let entry = generate_random_alphanumeric_string(20, charset);
    let post = format!("post_{}", entry);
    post
}


struct AccessPoints {
    pub chain1: ChainId,
    pub chain2: ChainId,
    pub node_service1: NodeService,
    pub node_service2: NodeService,
    pub app1: ApplicationWrapper<SocialAbi>,
    pub app2: ApplicationWrapper<SocialAbi>,
    pub notification2: Pin<Box<impl Stream<Item = Result<Notification>>>>,
    pub received_posts: Vec<String>,
}

impl AccessPoints {
    async fn check_posts(&self, context: String) -> anyhow::Result<()> {
        let query = "receivedPosts { keys { author, index } }";
        let value = self.app2.query(query).await?;
        println!("value={value}");
        let obj = value.as_object()?;
        let mut indices = BTreeSet::new();
        for (_, v) in obj {
            let s = match v {
                Value::Number(s) => indices.insert(s);
                _ => panic!("Should be a number");
            };
        }
        //
        let query = "ownPosts(entries { })";
        let mut received_posts = Vec::new();
        let value = self.app2.query(query).await?;
        let obj = value.as_object()?;
        for (_, v) in obj {
            let obj = v.as_object()?;
            let message = obj["text"].as_string();
            received_posts.push(message);
        }
        assert_eq!(received_posts, self.received_posts, "The received posts should match");
        assert_eq!(received_posts.len(), indices.len(), "The indices and received_posts length are not matching");
        Ok(())
    }

    async fn social_make_posts(&mut self) -> anyhow::Result<()> {
        self.check_posts("social_make_posts, beginning");
        let post = random_post();
        self.received_posts.push(post.clone());
        self.app1.mutate(format!("post(text: \"{post}\")"))
            .await?;
        let (_, height2) = self.node_service2.chain_tip(chain2).await?.unwrap();

        let author = format!("{chain1}");
        let query = "receivedPosts { keys { author, index } }";
        self.notifications2.wait_for_block(height2.try_add_one()?).await?;
        self.check_posts("social_make_posts, end");
        Ok(())
    }
}



/*
The test is adapted from the social test in linera-protocol.
But also of the reconfiguration test.
*/
async fn test_wasm_end_to_end_social_event_streams() -> anyhow::Result<()> {
    use social::SocialAbi;

    let config = get_config();
    let (mut net, client1) = config.instantiate().await?;

    let faucet_client = net.make_client().await;
    faucet_client.wallet_init(None).await?;

    let faucet_chain = client1
        .open_and_assign(&faucet_client, Amount::from_tokens(1_000u128))
        .await?;

    let mut faucet_service = faucet_client
        .run_faucet(None, Some(faucet_chain), Amount::from_tokens(2))
        .await?;

    faucet_service.ensure_is_running()?;

    let faucet = faucet_service.instance();

    assert_eq!(faucet.current_validators().await?.len(), 4);

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

    let mut notifications2 = node_service2.notifications(chain2).await?;

    let app1 = node_service1.make_application(&chain1, &application_id)?;

    //
    let access_points = AccessPoints {
        chain1,
        chain2,
        node_service1,
        node_service2,
        app1,
        app2,
        notifications2,
        received_posts: Vec::new(),
    };
    access_points.social_make_posts().await?;

    // Killing two validators. Restarting them with the moved code.
    net.terminate_server(2, 0).await?;
    net.terminate_server(3, 0).await?;
    net.terminate_proxy(2, 0).await?;
    net.terminate_proxy(3, 0).await?;

    net.directory = get_directory_new_schema();

    net.start_server(2, 0).await?;
    net.start_server(3, 0).await?;
    net.start_proxy(2, 0).await?;
    net.start_proxy(3, 0).await?;

    // Making the social posts. And checking
    access_points.social_make_posts().await?;

    // Killing the two remaining old validators. Restarting them with the moved code.
    net.terminate_server(0, 0).await?;
    net.terminate_server(1, 0).await?;
    net.terminate_proxy(0, 0).await?;
    net.terminate_proxy(1, 0).await?;

    net.start_server(0, 0).await?;
    net.start_server(1, 0).await?;
    net.start_proxy(0, 0).await?;
    net.start_proxy(1, 0).await?;

    // Making the social posts. And checking
    access_points.social_make_posts().await?;

    // Winding down.
    access_points.node_service1.ensure_is_running()?;
    access_points.node_service2.ensure_is_running()?;

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
