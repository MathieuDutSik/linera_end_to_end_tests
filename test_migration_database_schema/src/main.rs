use anyhow::Result;
use futures::Stream;
use linera_base::{
    data_types::Amount,
    identifiers::ChainId,
    time::{Duration, Instant},
    vm::VmRuntime,
};
use linera_core::worker::Notification;
use linera_service::{
    cli_wrappers::{
        local_net::{get_node_port, LocalNetConfig, ProcessInbox},
        ApplicationWrapper, ClientWrapper, LineraNet, LineraNetConfig, Network, NodeService,
    },
    util::eventually,
};
use linera_views::random::generate_random_alphanumeric_string;
use serde_json::Value;
use social::SocialAbi;

use std::{
    collections::BTreeSet,
    env,
    path::PathBuf,
    pin::Pin,
};

enum ChoiceVersion {
    V0,
    V1,
}

impl ChoiceVersion {
    fn get_dir(&self) -> String {
        match self {
            ChoiceVersion::V0 => format!("linera-protocol_test_conway_old_schema"),
            ChoiceVersion::V1 => format!("linera-protocol_test_conway_new_schema"),
        }
    }

    fn to_string(&self) -> String {
        format!("{}/target/debug", self.get_dir())
    }

    fn set_links(&self) {
        use std::os::unix::fs::symlink;
        let directory = self.to_string();
        for binary in ["linera", "linera-proxy", "linera-server", "linera-benchmark", "linera-spaceship", "linera-indexer"] {
            let target = env::current_dir().expect("pwd")
                .join(&directory)
                .join(binary);
            let link = env::current_dir().expect("pwd")
                .join("target/debug")
                .join(binary);
            if link.symlink_metadata().is_ok() {
                std::fs::remove_file(&link).unwrap();
            }
            symlink(target, link).expect("failed link creation");
        }
    }
}

fn get_directory(suffix: &str) -> String {
    let directory = std::env::current_dir().expect("directory").join(suffix);
    format!("{}", directory.display())
}

fn get_directory_old_schema() -> String {
    get_directory(&ChoiceVersion::V0.to_string())
}

fn get_directory_new_schema() -> String {
    get_directory(&ChoiceVersion::V1.to_string())
}

mod specified_local_net;
fn get_config_specified() -> specified_local_net::SpecifiedLocalNetConfig {
    let directory = get_directory_old_schema();
    println!("get_config, directory={directory}");
    let mut config =
        specified_local_net::SpecifiedLocalNetConfig::new_test(specified_local_net::Database::ScyllaDb, Network::Grpc, directory);
    config.num_initial_validators = 4;
    config.num_shards = 1;
    config
}

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(linera_service::cli_wrappers::local_net::Database::ScyllaDb, Network::Grpc);
    config.num_initial_validators = 4;
    config.num_shards = 1;
    config
}

async fn build_application(client: &ClientWrapper, name: &str) -> Result<(PathBuf, PathBuf)> {
    let path = env::current_dir()?
        .join("./smart_contract_code/")
        .join(name);
    Ok(client.build_application(&path, name, true).await?)
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
    pub received_posts: Vec<String>,
}

impl AccessPoints {
    async fn get_posts(&self, context1: &str, context2: &str) -> anyhow::Result<Vec<usize>> {
        println!("get_posts 1: context1={context1} context2={context2}");
        let query = "receivedPosts { keys { author, index } }";
        let value: Value = self.app2.query(query).await?;
        println!("get_posts 2: context1={context1} context2={context2}");
        println!("1: value={value}");
        let map = value.as_object().unwrap();
        let obj = map.get("receivedPosts").unwrap();
        println!("2: obj={obj}");
        let map = obj.as_object().unwrap();
        let obj = map.get("keys").unwrap();
        println!("3: obj={obj}");
        let mut indices = BTreeSet::new();
        let arr = obj.as_array().unwrap();
        for v in arr {
            println!("1: v={v}");
            let map = v.as_object().unwrap();
            let obj = map.get("index").unwrap();
            println!("2: obj={obj}");
            let v = obj.as_number().unwrap();
            let v = v.as_u64().unwrap();
            let v = v as usize;
            indices.insert(v);
        }
        let end = indices.len();
        //
//        let query = format!("ownPosts {{ entries(start: 0, end: {}) }}", end);
        let query = format!("ownPosts {{ entries {{ key, value {{ text }} }} }}");
        let mut received_posts: Vec<String> = Vec::new();
        println!("get_posts 3: context1={context1} context2={context2}");
        let value: Value = self.app2.query(query).await?;
        println!("value={value}");
        let map = value.as_object().unwrap();
        let obj = map.get("ownPosts").unwrap();
        println!("4: obj={obj}");
        let map = obj.as_object().unwrap();
        let obj = map.get("entries").unwrap();
        println!("5: obj={obj}");
        let arr = obj.as_array().unwrap();
        for v in arr {
            let obj = v.as_object().unwrap();
            let obj = obj.get("text").unwrap();
            let obj = obj.as_str().unwrap();
            let message = obj.to_string();
            received_posts.push(message);
        }
        println!("get_posts 4: context1={context1} context2={context2} received_posts={received_posts:?}");
        let indices = indices.into_iter().collect::<Vec<usize>>();
        Ok(indices)
    }

    async fn wait_process_inbox(&mut self, context1: &str) -> anyhow::Result<()> {
        for i in 0..5 {
            println!("wait_process_inbox 1: context1={context1}, i={i}");
            linera_base::time::timer::sleep(linera_base::time::Duration::from_secs(i)).await;
            println!("wait_process_inbox 2: context1={context1}, i={i}");
            let messages = self.node_service2.process_inbox(&self.chain2).await?;
            println!("wait_process_inbox 3: context1={context1}, i={i}");
            if messages.is_empty() {
                return Ok(())
            }
        }
        anyhow::bail!("Failed to get the message");
    }

    async fn check_posts(&mut self,
                         notifications2: &mut Pin<Box<impl Stream<Item = Result<Notification>>>>,
                         context1: &str) -> anyhow::Result<()> {
        /*

        
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut iter = 0;
        loop {
            let result =
                linera_base::time::timer::timeout(deadline - Instant::now(), notifications2.next())
                .await?;
            let context2 = format!("iter_{iter}");
            let received_posts = self.get_posts(context1, &context2).await?;
            if received_posts == self.received_posts {
                println!("Gotten the posts");
                break;
            }
            iter += 1;
         }
         */


        self.wait_process_inbox(&context1).await?;
        /*
        assert!(
            eventually(|| async {
                !self.node_service2
		    .process_inbox(&self.chain2)
                    .await
                    .unwrap()
                    .is_empty()
            }).await
        );
        */
        let indices = self.get_posts(context1, "check_posts").await?;
        assert_eq!(indices.len(), self.received_posts.len());

        Ok(())
    }

    async fn social_make_posts(
        &mut self,
        notifications2: &mut Pin<Box<impl Stream<Item = Result<Notification>>>>,
        context1: &str,
    ) -> anyhow::Result<()> {
        println!("social_make_posts 1: context1={context1}");
        self.check_posts(notifications2, context1).await?;
        println!("social_make_posts 2: context1={context1}");
        let post = random_post();
        self.received_posts.push(post.clone());
        self.app1.mutate(format!("post(text: \"{post}\")")).await?;
        println!("social_make_posts 3: context1={context1}");
        self.check_posts(notifications2, context1).await?;
        println!("social_make_posts 4: context1={context1}");
        Ok(())
    }
}

/*
The test is adapted from the social test in linera-protocol.
But also of the reconfiguration test.
*/
async fn test_wasm_end_to_end_social_event_streams() -> anyhow::Result<()> {
    use social::SocialAbi;

    ChoiceVersion::V0.set_links();
    let config = get_config_specified();
    let (mut net, client1) = config.instantiate().await?;

    let faucet_client = net.make_client().await;
    faucet_client.wallet_init(None).await?;

    let faucet_chain = client1
        .open_and_assign(&faucet_client, Amount::from_tokens(1_000u128))
        .await?;

    let mut faucet_service = faucet_client
        .run_faucet(None, faucet_chain, Amount::from_tokens(2))
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
    let node_service1 = client1
        .run_node_service(port1, ProcessInbox::Automatic)
        .await?;
    let node_service2 = client2
        .run_node_service(port2, ProcessInbox::Automatic)
        .await?;

    let app2 = node_service2.make_application(&chain2, &application_id)?;
    app2.mutate(format!("subscribe(chainId: \"{chain1}\")"))
        .await?;
    let mut notifications2 = Box::pin(node_service2.notifications(chain2).await?);

    let app1 = node_service1.make_application(&chain1, &application_id)?;

    //
    let mut access_points = AccessPoints {
        chain1,
        chain2,
        node_service1,
        node_service2,
        app1,
        app2,
        received_posts: Vec::new(),
    };
    access_points.social_make_posts(&mut notifications2, "First post").await?;

    // Killing two validators. Restarting them with the moved code.
    net.stop_validator(2).await?;
    net.stop_validator(3).await?;

    ChoiceVersion::V1.set_links();

    println!("test_wasm_end_to_end_social_event_streams, step 10");
    net.restart_validator(2).await?;
    println!("test_wasm_end_to_end_social_event_streams, step 11");
    net.restart_validator(3).await?;
    println!("test_wasm_end_to_end_social_event_streams, step 12");

    // Making the social posts. And checking
    access_points.social_make_posts(&mut notifications2, "Second post").await?;
    println!("test_wasm_end_to_end_social_event_streams, step 13");
    /*

    // Killing the two remaining old validators. Restarting them with the moved code.
    net.stop_validator(0).await?;
    net.stop_validator(1).await?;

    net.restart_validator(0).await?;
    net.restart_validator(1).await?;

    // Making the social posts. And checking
    access_points.social_make_posts(&mut notifications2, "Third post").await?;

    */
    // Winding down.
    access_points.node_service1.ensure_is_running()?;
    access_points.node_service2.ensure_is_running()?;

    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Normal termination of the test");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("main, step 1");
    let args: Vec<String> = env::args().collect();
    println!("main, step 2");

    if args.len() < 2 {
        eprintln!("Error: No test specified");
        eprintln!("Usage: {} <test-name>", args[0]);
        std::process::exit(1);
    }
    println!("main, step 3");

    let test_name = &args[1];
    println!("test_name={test_name}");

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
