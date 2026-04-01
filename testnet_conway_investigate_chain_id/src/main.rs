use anyhow::Result;
use clap::Parser;
use linera_base::{
    crypto::CryptoHash,
    data_types::{Amount, BlockHeight},
    identifiers::ChainId,
};
use linera_chain::data_types::IncomingBundle;
use linera_core::{
    data_types::ChainInfoQuery,
    node::{ValidatorNode, ValidatorNodeProvider},
};
use linera_execution::Message;
use testnet_conway_investigate_chain_id::VALIDATORS;

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

    /// Show chain owners (from manager ownership info).
    #[arg(long)]
    owners: bool,

    /// Request received log (chain IDs and heights of received certificates).
    /// Optionally exclude the first N entries.
    #[arg(long, value_name = "EXCLUDE_FIRST_N", default_missing_value = "0", num_args = 0..=1)]
    received_log: Option<u64>,

    /// Request all available info (enables all flags).
    #[arg(long)]
    all: bool,
}

struct ValidatorResult {
    address: String,
    next_block_height: BlockHeight,
    chain_balance: Amount,
    block_hash: Option<CryptoHash>,
    state_hash: Option<CryptoHash>,
}

fn print_owners(info: &linera_core::data_types::ChainInfo) {
    let ownership = &info.manager.ownership;
    if !ownership.super_owners.is_empty() {
        println!("  Super owners ({}):", ownership.super_owners.len());
        for owner in &ownership.super_owners {
            println!("    {}", owner);
        }
    }
    if !ownership.owners.is_empty() {
        println!("  Owners ({}):", ownership.owners.len());
        for (owner, weight) in &ownership.owners {
            println!("    {} (weight: {})", owner, weight);
        }
    }
    if ownership.super_owners.is_empty() && ownership.owners.is_empty() {
        println!("  Owners:             none (public chain)");
    }
    println!(
        "  Multi-leader rounds: {} (open: {})",
        ownership.multi_leader_rounds, ownership.open_multi_leader_rounds
    );
    let tc = &ownership.timeout_config;
    println!(
        "  Timeout config:      base={:?}, increment={:?}, fallback={:?}, fast_round={:?}",
        tc.base_timeout, tc.timeout_increment, tc.fallback_duration, tc.fast_round_duration
    );
}

fn print_manager_info(info: &linera_core::data_types::ChainInfo) {
    let mgr = &info.manager;
    println!("  Current round:      {:?}", mgr.current_round);
    if let Some(leader) = &mgr.leader {
        println!("  Current leader:     {}", leader);
    }
    if let Some(timeout) = &mgr.round_timeout {
        let datetime =
            testnet_conway_investigate_chain_id::micros_to_datetime_string(timeout.micros());
        println!("  Round timeout:      {:?} ({})", timeout, datetime);
    }
    if mgr.pending.is_some() {
        println!("  Pending vote:       yes");
    }
    if mgr.timeout_vote.is_some() {
        println!("  Timeout vote:       yes");
    }
    if mgr.fallback_vote.is_some() {
        println!("  Fallback vote:      yes");
    }
    if mgr.timeout.is_some() {
        println!("  Timeout cert:       present");
    }
}

fn print_pending_bundle(bundle: &IncomingBundle) {
    let datetime = testnet_conway_investigate_chain_id::micros_to_datetime_string(
        bundle.bundle.timestamp.micros(),
    );
    println!(
        "    Origin: {}, height: {}, time: {} ({:?}), cert: {}, action: {:?}",
        bundle.origin,
        bundle.bundle.height,
        datetime,
        bundle.bundle.timestamp,
        bundle.bundle.certificate_hash,
        bundle.action,
    );
    for msg in bundle.bundle.messages.iter() {
        let app_info = match &msg.message {
            Message::System(sys_msg) => format!("System({:?})", sys_msg),
            Message::User {
                application_id,
                bytes,
            } => format!(
                "User(app: {}, {} bytes)",
                application_id,
                bytes.len()
            ),
        };
        println!(
            "      msg[{}]: kind={}, grant={}, {}",
            msg.index, msg.kind, msg.grant, app_info,
        );
        if let Some(signer) = &msg.authenticated_signer {
            println!("        signer: {}", signer);
        }
    }
}

fn print_info(
    info: &linera_core::data_types::ChainInfo,
    signature: &Option<linera_base::crypto::ValidatorSignature>,
    show_owners: bool,
) {
    println!("  Chain ID:           {}", info.chain_id);
    println!("  Epoch:              {:?}", info.epoch);
    if let Some(desc) = &info.description {
        println!("  Description:        {:?}", desc);
    }
    println!("  Next block height:  {}", info.next_block_height);
    println!("  Chain balance:      {}", info.chain_balance);
    let datetime = testnet_conway_investigate_chain_id::micros_to_datetime_string(
        info.timestamp.micros(),
    );
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

    // Manager info (always available)
    print_manager_info(info);

    // Owners
    if show_owners {
        print_owners(info);
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
        for bundle in &info.requested_pending_message_bundles {
            print_pending_bundle(bundle);
        }
        // Collect unique application IDs from pending messages
        let mut app_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for bundle in &info.requested_pending_message_bundles {
            for msg in bundle.bundle.messages.iter() {
                match &msg.message {
                    Message::System(_) => {
                        app_ids.insert("System".to_string());
                    }
                    Message::User { application_id, .. } => {
                        app_ids.insert(format!("{}", application_id));
                    }
                }
            }
        }
        if !app_ids.is_empty() {
            println!(
                "  Applications in pending messages ({}):",
                app_ids.len()
            );
            for app_id in &app_ids {
                println!("    {}", app_id);
            }
        }
    }
    if !info.requested_received_log.is_empty() {
        println!(
            "  Received log entries: {}",
            info.requested_received_log.len()
        );
        for entry in &info.requested_received_log {
            println!(
                "    from chain {} at height {}",
                entry.chain_id, entry.height
            );
        }
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
    testnet_conway_investigate_chain_id::init_tracing();
    testnet_conway_investigate_chain_id::init_rustls();

    let args = Args::parse();
    let chain_id: ChainId = args.chain_id.parse()?;

    let provider = testnet_conway_investigate_chain_id::default_node_provider();

    let mut results: Vec<ValidatorResult> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    let show_owners = args.owners || args.all;

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
        if args.committees || args.all {
            query = query.with_committees();
        }
        if args.pending_messages || args.all {
            query = query.with_pending_message_bundles();
        }
        if args.manager_values || args.all {
            query = query.with_manager_values();
        }
        if let Some(exclude_n) = args.received_log {
            query = query.with_received_log_excluding_first_n(exclude_n);
        } else if args.all {
            query = query.with_received_log_excluding_first_n(0);
        }

        match node.handle_chain_info_query(query).await {
            Ok(response) => {
                print_info(&response.info, &response.signature, show_owners);
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
