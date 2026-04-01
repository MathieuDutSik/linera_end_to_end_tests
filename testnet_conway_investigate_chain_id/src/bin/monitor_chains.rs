use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use linera_base::data_types::BlockHeight;
use linera_base::identifiers::ChainId;
use linera_core::data_types::ChainInfoQuery;
use linera_core::node::{ValidatorNode, ValidatorNodeProvider};
use testnet_conway_investigate_chain_id::{
    VALIDATORS, default_node_provider, init_rustls, init_tracing, micros_to_datetime_string,
};

#[derive(Parser)]
#[command(about = "Monitor multiple chains across validators for health issues")]
struct Args {
    /// Path to a file containing one chain ID per line.
    chains_file: PathBuf,

    /// Block height difference threshold to flag as a big discrepancy.
    #[arg(long, default_value = "5")]
    big_diff_threshold: u64,

    /// Number of seconds in the past beyond which a timestamp is considered stale.
    #[arg(long, default_value = "300")]
    stale_seconds: u64,
}

struct ValidatorChainInfo {
    block_height: BlockHeight,
    timestamp_micros: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    init_rustls();

    let args = Args::parse();

    let contents = std::fs::read_to_string(&args.chains_file)
        .with_context(|| format!("Failed to read {}", args.chains_file.display()))?;
    let chain_ids: Vec<ChainId> = contents
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.parse::<ChainId>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to parse chain IDs")?;

    if chain_ids.is_empty() {
        println!("No chain IDs found in file.");
        return Ok(());
    }

    println!("Monitoring {} chains across {} validators\n", chain_ids.len(), VALIDATORS.len());

    let provider = default_node_provider();
    let now_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;

    // For tracking small-but-consistent diffs: map from (chain, lagging_validator) -> count
    let mut lagging_validators: BTreeMap<String, u64> = BTreeMap::new();
    let mut issues_found = false;

    for chain_id in &chain_ids {
        let mut infos: Vec<(String, ValidatorChainInfo)> = Vec::new();
        let mut errors: Vec<(String, String)> = Vec::new();

        for address in VALIDATORS {
            let node = match provider.make_node(address) {
                Ok(n) => n,
                Err(e) => {
                    errors.push((address.to_string(), format!("{e}")));
                    continue;
                }
            };

            let query = ChainInfoQuery::new(*chain_id);
            match node.handle_chain_info_query(query).await {
                Ok(response) => {
                    infos.push((
                        address.to_string(),
                        ValidatorChainInfo {
                            block_height: response.info.next_block_height,
                            timestamp_micros: response.info.timestamp.micros(),
                        },
                    ));
                }
                Err(e) => {
                    errors.push((address.to_string(), format!("{e}")));
                }
            }
        }

        if !errors.is_empty() {
            issues_found = true;
            println!("CHAIN {} — query errors:", chain_id);
            for (addr, err) in &errors {
                println!("  {} : {}", addr, err);
            }
            println!();
        }

        if infos.is_empty() {
            continue;
        }

        // Check block height discrepancies
        let max_height = infos.iter().map(|(_, i)| i.block_height).max().unwrap();
        let min_height = infos.iter().map(|(_, i)| i.block_height).min().unwrap();
        let diff = max_height
            .0
            .checked_sub(min_height.0)
            .unwrap_or(0);

        if diff >= args.big_diff_threshold {
            issues_found = true;
            println!(
                "CHAIN {} — BIG block height discrepancy (max={}, min={}, diff={}):",
                chain_id, max_height, min_height, diff
            );
            for (addr, info) in &infos {
                let marker = if info.block_height == min_height { " <-- LAGGING" } else { "" };
                println!("  {} : height {}{}",
                    addr, info.block_height, marker);
            }
            println!();
        } else if diff > 0 {
            // Small difference — track which validators are behind
            for (addr, info) in &infos {
                if info.block_height < max_height {
                    let key = addr.clone();
                    *lagging_validators.entry(key).or_insert(0) += 1;
                }
            }
        }

        // Check stale timestamps
        for (addr, info) in &infos {
            if info.timestamp_micros == 0 {
                continue; // Genesis or uninitialized chain
            }
            let age_secs = now_micros.saturating_sub(info.timestamp_micros) / 1_000_000;
            if age_secs > args.stale_seconds {
                issues_found = true;
                let datetime = micros_to_datetime_string(info.timestamp_micros);
                println!(
                    "CHAIN {} — STALE timestamp on {} : last block at {} ({} seconds ago)",
                    chain_id, addr, datetime, age_secs
                );
            }
        }
    }

    // Report validators that are consistently slightly behind
    if !lagging_validators.is_empty() {
        let total_chains = chain_ids.len() as u64;
        println!("\n=== Validators with small but recurring lag ===");
        for (validator, count) in &lagging_validators {
            let pct = (*count as f64 / total_chains as f64) * 100.0;
            println!(
                "  {} : behind on {}/{} chains ({:.0}%)",
                validator, count, total_chains, pct
            );
            if *count > total_chains / 2 {
                issues_found = true;
            }
        }
        println!();
    }

    if !issues_found {
        println!("All chains healthy: block heights consistent, timestamps recent.");
    }

    Ok(())
}
