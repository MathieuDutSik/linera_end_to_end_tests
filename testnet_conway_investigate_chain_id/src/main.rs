use anyhow::Result;
use clap::Parser;
use linera_base::{
    crypto::CryptoHash,
    data_types::{Amount, BlockHeight},
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

    /// Check whether all validators return the same results.
    #[arg(long)]
    check_consistency: bool,

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

struct ValidatorResult {
    address: String,
    next_block_height: BlockHeight,
    chain_balance: Amount,
    block_hash: Option<CryptoHash>,
    state_hash: Option<CryptoHash>,
}

fn print_info(
    info: &linera_core::data_types::ChainInfo,
    signature: &Option<linera_base::crypto::ValidatorSignature>,
) {
    println!("  Chain ID:           {}", info.chain_id);
    println!("  Epoch:              {:?}", info.epoch);
    println!("  Next block height:  {}", info.next_block_height);
    println!("  Chain balance:      {}", info.chain_balance);
    let micros = info.timestamp.micros();
    let secs = (micros / 1_000_000) as i64;
    let sub_micros = (micros % 1_000_000) as u32;
    let datetime = chrono::DateTime::from_timestamp(secs, sub_micros * 1000)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.6f UTC").to_string())
        .unwrap_or_else(|| "invalid".to_string());
    println!("  Timestamp:          {:?} ({})", info.timestamp, datetime);
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
    if signature.is_some() {
        println!("  Signature:          present");
    } else {
        println!("  Signature:          none");
    }
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

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

    let mut results: Vec<ValidatorResult> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    for address in VALIDATORS {
        println!("=== Querying {} ===", address);
        let node = match provider.make_node(address) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("  Failed to create node: {e}");
                errors.push((address.to_string(), format!("{e}")));
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
                print_info(&response.info, &response.signature);
                results.push(ValidatorResult {
                    address: address.to_string(),
                    next_block_height: response.info.next_block_height,
                    chain_balance: response.info.chain_balance,
                    block_hash: response.info.block_hash,
                    state_hash: response.info.state_hash,
                });
            }
            Err(e) => {
                eprintln!("  Error: {e}");
                errors.push((address.to_string(), format!("{e}")));
                println!();
            }
        }
    }

    if args.check_consistency {
        println!("=== Consistency check ===");
        if results.is_empty() {
            println!("  No successful responses to compare.");
        } else if results.len() == 1 {
            println!("  Only one validator responded, nothing to compare.");
        } else {
            let first = &results[0];
            let mut consistent = true;

            for r in &results[1..] {
                let mut diffs = Vec::new();
                if r.next_block_height != first.next_block_height {
                    diffs.push(format!(
                        "next_block_height: {} vs {}",
                        first.next_block_height, r.next_block_height
                    ));
                }
                if r.chain_balance != first.chain_balance {
                    diffs.push(format!(
                        "chain_balance: {} vs {}",
                        first.chain_balance, r.chain_balance
                    ));
                }
                if r.block_hash != first.block_hash {
                    diffs.push(format!(
                        "block_hash: {:?} vs {:?}",
                        first.block_hash, r.block_hash
                    ));
                }
                if r.state_hash != first.state_hash {
                    diffs.push(format!(
                        "state_hash: {:?} vs {:?}",
                        first.state_hash, r.state_hash
                    ));
                }
                if !diffs.is_empty() {
                    consistent = false;
                    println!(
                        "  MISMATCH between {} and {}:",
                        first.address, r.address
                    );
                    for d in &diffs {
                        println!("    {}", d);
                    }
                }
            }

            if !errors.is_empty() {
                consistent = false;
                println!("  ERRORS from validators:");
                for (addr, err) in &errors {
                    println!("    {}: {}", addr, err);
                }
            }

            if consistent {
                println!("  All {} validators agree.", results.len());
            }
        }
    }

    Ok(())
}
