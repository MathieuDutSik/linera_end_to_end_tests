use anyhow::Result;
use clap::Parser;
use linera_base::{
    identifiers::ChainId,
    time::Duration,
};
use linera_core::{
    data_types::ChainInfoQuery,
    node::{ValidatorNode, ValidatorNodeProvider},
};
use linera_rpc::{NodeOptions, NodeProvider};

const VALIDATORS: &[&str] = &[
    "grpcs:validator-1.testnet-conway.linera.net:443",
    "grpcs:validator-2.testnet-conway.linera.net:443",
    "grpcs:validator-3.testnet-conway.linera.net:443",
    "grpcs:validator-4.testnet-conway.linera.net:443",
];

#[derive(Parser)]
#[command(about = "Query chain info from Conway testnet validators")]
struct Args {
    /// The chain ID to query (hex string).
    chain_id: String,

    /// Request committees info.
    #[arg(long)]
    committees: bool,

    /// Request pending message bundles.
    #[arg(long)]
    pending_messages: bool,

    /// Request manager values.
    #[arg(long)]
    manager_values: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let args = Args::parse();
    let chain_id: ChainId = args.chain_id.parse()?;

    let options = NodeOptions {
        send_timeout: Duration::from_secs(30),
        recv_timeout: Duration::from_secs(30),
        retry_delay: Duration::from_millis(500),
        max_retries: 3,
        max_backoff: Duration::from_secs(10),
    };
    let provider = NodeProvider::new(options);

    for address in VALIDATORS {
        println!("=== Querying {} ===", address);
        let node = match provider.make_node(address) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("  Failed to create node: {e}");
                continue;
            }
        };

        let mut query = ChainInfoQuery::new(chain_id);
        if args.committees {
            query = query.with_committees();
        }
        if args.pending_messages {
            query = query.with_pending_message_bundles();
        }
        if args.manager_values {
            query = query.with_manager_values();
        }

        match node.handle_chain_info_query(query).await {
            Ok(response) => {
                let info = response.info;
                println!("  Chain ID:           {}", info.chain_id);
                println!("  Epoch:              {:?}", info.epoch);
                println!("  Next block height:  {}", info.next_block_height);
                println!("  Chain balance:      {}", info.chain_balance);
                println!("  Timestamp:          {:?}", info.timestamp);
                if let Some(hash) = &info.block_hash {
                    println!("  Block hash:         {}", hash);
                }
                if let Some(state_hash) = &info.state_hash {
                    println!("  State hash:         {}", state_hash);
                }
                if let Some(balance) = &info.requested_owner_balance {
                    println!("  Owner balance:      {}", balance);
                }
                if let Some(ref committees) = info.requested_committees {
                    println!("  Committees:         {} epoch(s)", committees.len());
                    for (epoch, _committee) in committees {
                        println!("    Epoch {:?}", epoch);
                    }
                }
                if !info.requested_pending_message_bundles.is_empty() {
                    println!(
                        "  Pending bundles:    {}",
                        info.requested_pending_message_bundles.len()
                    );
                }
                println!("  Received log count: {}", info.count_received_log);
                if response.signature.is_some() {
                    println!("  Signature:          present");
                } else {
                    println!("  Signature:          none");
                }
                println!();
            }
            Err(e) => {
                eprintln!("  Error: {e}");
                println!();
            }
        }
    }

    Ok(())
}
