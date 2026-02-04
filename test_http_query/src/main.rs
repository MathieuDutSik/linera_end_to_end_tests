use anyhow::Result;
use http_query::{HttpQueryAbi, HttpQueryRequest};
use linera_base::vm::VmRuntime;
use linera_service::cli_wrappers::{
    local_net::{get_node_port, Database, LocalNetConfig, ProcessInbox},
    LineraNet, LineraNetConfig, Network,
};

const COINBASE_URL: &str = "https://api.exchange.coinbase.com/products/BTC-USD/trades";
const KRAKEN_URL: &str = "https://api.kraken.com/0/public/Trades?pair=XBTUSDT";

fn get_config() -> LocalNetConfig {
    let mut config = LocalNetConfig::new_test(Database::Service, Network::Grpc);
    config.num_initial_validators = 1;
    config.num_shards = 1;
//    config.http_request_allow_list = Some(vec![
//        "localhost".to_owned(),
//        "api.exchange.coinbase.com".to_owned(),
//        "api.kraken.com".to_owned(),
//    ]);
    config.http_request_allow_list = Some(vec![
        "api.exchange.coinbase.com".to_owned(),
    ]);
    config
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = get_config();

    let (mut net, client) = config.instantiate().await?;
    let chain = client.load_wallet()?.default_chain().unwrap();

    let name1 = "http-query";
    let path1 = std::env::current_dir()?.join("./smart_contract_code/").join(name1);
    let (contract_path, service_path) =
        client.build_application(&path1, name1, true).await?;

    let application_id = client
        .publish_and_create::<HttpQueryAbi, (), ()>(
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
    let application = node_service.make_application(&chain, &application_id)?;

    let coinbase_value = application
        .run_json_query(HttpQueryRequest::HttpGet(COINBASE_URL.to_owned()))
        .await?;
    let coinbase_len = coinbase_value
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unexpected Coinbase response: {coinbase_value}"))?;
    anyhow::ensure!(coinbase_len > 0, "Coinbase response length is zero");

/*
    let kraken_value = application
        .run_json_query(HttpQueryRequest::HttpGet(KRAKEN_URL.to_owned()))
        .await?;
    let kraken_len = kraken_value
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unexpected Kraken response: {kraken_value}"))?;
    anyhow::ensure!(kraken_len > 0, "Kraken response length is zero");
*/

    node_service.ensure_is_running()?;
    net.ensure_is_running().await?;
    net.terminate().await?;
    println!("Normal termination of the program");
    Ok(())
}
