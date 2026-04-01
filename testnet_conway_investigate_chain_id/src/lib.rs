use linera_base::time::Duration;
use linera_rpc::{NodeOptions, NodeProvider};

pub const VALIDATORS: &[&str] = &[
    "grpcs:validator-1.testnet-conway.linera.net:443",
    "grpcs:validator-2.testnet-conway.linera.net:443",
    "grpcs:validator-3.testnet-conway.linera.net:443",
    "grpcs:validator-4.testnet-conway.linera.net:443",
];

pub fn default_node_provider() -> NodeProvider {
    let options = NodeOptions {
        send_timeout: Duration::from_secs(30),
        recv_timeout: Duration::from_secs(30),
        retry_delay: Duration::from_millis(500),
        max_retries: 3,
        max_backoff: Duration::from_secs(10),
    };
    NodeProvider::new(options)
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();
}

pub fn init_rustls() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
}

pub fn micros_to_datetime_string(micros: u64) -> String {
    let secs = (micros / 1_000_000) as i64;
    let sub_micros = (micros % 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, sub_micros * 1000)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "invalid".to_string())
}
