use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use linera_base::identifiers::{AccountOwner, ChainId};
use linera_faucet_client::Faucet;
use serde::de::DeserializeOwned;

const DEFAULT_FAUCET_URL: &str = "https://faucet.testnet-conway.linera.net";

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Faucet URL. Defaults to the `testnet_conway` faucet from `remote_compatibility.yml`.
    #[arg(long, default_value = DEFAULT_FAUCET_URL)]
    faucet_url: String,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print the admin chain ID derived from the faucet's genesis config.
    AdminChainId,
    /// Print a short summary about the remote faucet.
    Info,
    /// Query the chain ID associated with an owner that has already claimed a chain.
    OwnerChainId {
        /// Account owner string accepted by the faucet GraphQL API.
        owner: AccountOwner,
    },
}

#[derive(Debug, serde::Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize)]
struct OwnerChainIdResponse {
    #[serde(rename = "chainId")]
    chain_id: ChainId,
}

async fn graph_ql_query<Response>(url: &str, query: impl Into<String>) -> Result<Response>
where
    Response: DeserializeOwned,
{
    let query = query.into();
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?
        .post(url)
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .with_context(|| format!("failed to execute GraphQL query against {url}"))?
        .error_for_status()
        .context("faucet returned a non-success HTTP status")?
        .json::<GraphQlResponse<Response>>()
        .await
        .context("failed to deserialize GraphQL response")?;

    if let Some(errors) = response.errors {
        bail!("faucet GraphQL error: {}", serde_json::Value::Array(errors));
    }

    response
        .data
        .ok_or_else(|| anyhow!("faucet returned no data and no GraphQL errors"))
}

async fn owner_chain_id(url: &str, owner: AccountOwner) -> Result<ChainId> {
    let response: OwnerChainIdResponse =
        graph_ql_query(url, format!("query {{ chainId(owner: \"{owner}\") }}")).await?;
    Ok(response.chain_id)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let faucet = Faucet::new(cli.faucet_url.clone());

    match cli.command.unwrap_or(Command::Info) {
        Command::AdminChainId => {
            let genesis = faucet
                .genesis_config()
                .await
                .with_context(|| format!("failed to fetch genesis config from {}", faucet.url()))?;
            println!("{}", genesis.admin_chain_id());
        }
        Command::Info => {
            let version = faucet
                .version_info()
                .await
                .with_context(|| format!("failed to fetch version info from {}", faucet.url()))?;
            let epoch = graph_ql_query::<serde_json::Value>(faucet.url(), "query { currentEpoch }")
                .await
                .with_context(|| format!("failed to fetch current epoch from {}", faucet.url()))?;
            let genesis = faucet
                .genesis_config()
                .await
                .with_context(|| format!("failed to fetch genesis config from {}", faucet.url()))?;

            println!("faucet_url={}", faucet.url());
            println!("version={version:?}");
            println!("current_epoch={}", epoch["currentEpoch"]);
            println!("admin_chain_id={}", genesis.admin_chain_id());
            println!(
                "note=the public faucet API exposes the network genesis config and owner-specific claimed chain IDs; it does not expose the faucet service chain ID directly"
            );
        }
        Command::OwnerChainId { owner } => {
            let chain_id = owner_chain_id(faucet.url(), owner).await?;
            println!("{chain_id}");
        }
    }

    Ok(())
}
