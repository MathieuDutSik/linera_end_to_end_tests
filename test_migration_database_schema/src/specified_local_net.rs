// Copyright (c) Zefchain Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, env, path::Path, time::Duration};

use anyhow::{anyhow, bail, ensure, Context, Result};
use async_trait::async_trait;
use linera_base::{command::CommandExt, data_types::Amount};
use linera_client::client_options::ResourceControlPolicyConfig;
use linera_core::node::ValidatorNodeProvider;
use linera_rpc::config::CrossChainConfig;
use linera_storage_service::common::storage_service_test_endpoint;
use linera_views::{scylla_db::ScyllaDbDatabase, store::TestKeyValueDatabase as _};
use tokio::process::{Child, Command};
use tonic::transport::{channel::ClientTlsConfig, Endpoint};
use tonic_health::pb::{
    health_check_response::ServingStatus, health_client::HealthClient, HealthCheckRequest,
};
use tracing::{error, info, warn};

use linera_service::{
    cli_wrappers::{
        local_net::PathProvider, ClientWrapper, LineraNet, LineraNetConfig, Network, NetworkConfig,
        OnClientDrop,
    },
    storage::{InnerStorageConfig, StorageConfig},
    util::ChildExt,
};

/// Maximum allowed number of shards over all validators.
const MAX_NUMBER_SHARDS: usize = 1000;

pub const FIRST_PUBLIC_PORT: usize = 13000;

async fn make_testing_config(database: Database) -> Result<InnerStorageConfig> {
    match database {
        Database::Service => {
            let endpoint = storage_service_test_endpoint()
                .expect("Reading LINERA_STORAGE_SERVICE environment variable");
            Ok(InnerStorageConfig::Service { endpoint })
        }
        Database::ScyllaDb => {
            let config = ScyllaDbDatabase::new_test_config().await?;
            Ok(InnerStorageConfig::ScyllaDb {
                uri: config.inner_config.uri,
            })
        }
    }
}

pub enum InnerStorageConfigBuilder {
    TestConfig,
}

impl InnerStorageConfigBuilder {
    pub async fn build(self, database: Database) -> Result<InnerStorageConfig> {
        match self {
            InnerStorageConfigBuilder::TestConfig => make_testing_config(database).await,
        }
    }
}

/// The information needed to start a [`SpecifiedLocalNet`].
pub struct SpecifiedLocalNetConfig {
    pub database: Database,
    pub network: NetworkConfig,
    pub directory: String,
    pub testing_prng_seed: Option<u64>,
    pub namespace: String,
    pub num_other_initial_chains: u32,
    pub initial_amount: Amount,
    pub num_initial_validators: usize,
    pub num_shards: usize,
    pub num_proxies: usize,
    pub policy_config: ResourceControlPolicyConfig,
    pub cross_chain_config: CrossChainConfig,
    pub storage_config_builder: InnerStorageConfigBuilder,
    pub path_provider: PathProvider,
}

/// A set of Linera validators running locally as native processes.
pub struct SpecifiedLocalNet {
    network: NetworkConfig,
    pub directory: String,
    testing_prng_seed: Option<u64>,
    next_client_id: usize,
    num_initial_validators: usize,
    num_proxies: usize,
    num_shards: usize,
    validator_keys: BTreeMap<usize, (String, String)>,
    running_validators: BTreeMap<usize, Validator>,
    initialized_validator_storages: BTreeMap<usize, StorageConfig>,
    common_namespace: String,
    common_storage_config: InnerStorageConfig,
    cross_chain_config: CrossChainConfig,
    path_provider: PathProvider,
}

/// The name of the environment variable that allows specifying additional arguments to be passed
/// to the binary when starting a server.
const SERVER_ENV: &str = "LINERA_SERVER_PARAMS";

/// Description of the database engine to use inside a local Linera network.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Database {
    Service,
    ScyllaDb,
}

/// The processes of a running validator.
struct Validator {
    proxies: Vec<Child>,
    servers: Vec<Child>,
}

impl Validator {
    fn new() -> Self {
        Self {
            proxies: vec![],
            servers: vec![],
        }
    }

    async fn terminate(&mut self) -> Result<()> {
        for proxy in &mut self.proxies {
            proxy.kill().await.context("terminating validator proxy")?;
        }
        for server in &mut self.servers {
            server
                .kill()
                .await
                .context("terminating validator server")?;
        }
        Ok(())
    }

    fn add_proxy(&mut self, proxy: Child) {
        self.proxies.push(proxy)
    }

    fn add_server(&mut self, server: Child) {
        self.servers.push(server)
    }

    async fn terminate_server(&mut self, index: usize) -> Result<()> {
        let mut server = self.servers.remove(index);
        server
            .kill()
            .await
            .context("terminating validator server")?;
        Ok(())
    }

    fn ensure_is_running(&mut self) -> Result<()> {
        for proxy in &mut self.proxies {
            proxy.ensure_is_running()?;
        }
        for child in &mut self.servers {
            child.ensure_is_running()?;
        }
        Ok(())
    }
}

impl SpecifiedLocalNetConfig {
    pub fn new_test(database: Database, network: Network, directory: String) -> Self {
        let num_shards = 4;
        let num_proxies = 1;
        let storage_config_builder = InnerStorageConfigBuilder::TestConfig;
        let path_provider = PathProvider::create_temporary_directory().unwrap();
        let internal = network.drop_tls();
        let external = network;
        let network = NetworkConfig { internal, external };
        let cross_chain_config = CrossChainConfig::default();
        Self {
            database,
            network,
            directory,
            num_other_initial_chains: 2,
            initial_amount: Amount::from_tokens(1_000_000),
            policy_config: ResourceControlPolicyConfig::Testnet,
            cross_chain_config,
            testing_prng_seed: Some(37),
            namespace: linera_views::random::generate_test_namespace(),
            num_initial_validators: 4,
            num_shards,
            num_proxies,
            storage_config_builder,
            path_provider,
        }
    }
}

#[async_trait]
impl LineraNetConfig for SpecifiedLocalNetConfig {
    type Net = SpecifiedLocalNet;

    async fn instantiate(self) -> Result<(Self::Net, ClientWrapper)> {
        let storage_config = self.storage_config_builder.build(self.database).await?;
        let mut net = SpecifiedLocalNet::new(
            self.network,
            self.directory,
            self.testing_prng_seed,
            self.namespace,
            self.num_initial_validators,
            self.num_proxies,
            self.num_shards,
            storage_config,
            self.cross_chain_config,
            self.path_provider,
        );
        let client = net.make_client().await;
        ensure!(
            self.num_initial_validators > 0,
            "There should be at least one initial validator"
        );
        let total_number_shards = self.num_initial_validators * self.num_shards;
        ensure!(
            total_number_shards <= MAX_NUMBER_SHARDS,
            "Total number of shards ({}) exceeds maximum allowed ({})",
            self.num_shards,
            MAX_NUMBER_SHARDS
        );
        net.generate_initial_validator_config().await?;
        client
            .create_genesis_config(
                self.num_other_initial_chains,
                self.initial_amount,
                self.policy_config,
                Some(vec!["localhost".to_owned()]),
            )
            .await?;
        net.run().await?;
        Ok((net, client))
    }
}

#[async_trait]
impl LineraNet for SpecifiedLocalNet {
    async fn ensure_is_running(&mut self) -> Result<()> {
        for validator in self.running_validators.values_mut() {
            validator.ensure_is_running().context("in local network")?;
        }
        Ok(())
    }

    async fn make_client(&mut self) -> ClientWrapper {
        let client = ClientWrapper::new(
            self.path_provider.clone(),
            self.network.external,
            self.testing_prng_seed,
            self.next_client_id,
            OnClientDrop::LeakChains,
        );
        if let Some(seed) = self.testing_prng_seed {
            self.testing_prng_seed = Some(seed + 1);
        }
        self.next_client_id += 1;
        client
    }

    async fn terminate(&mut self) -> Result<()> {
        for validator in self.running_validators.values_mut() {
            validator.terminate().await.context("in local network")?
        }
        Ok(())
    }
}

impl SpecifiedLocalNet {
    #[expect(clippy::too_many_arguments)]
    fn new(
        network: NetworkConfig,
        directory: String,
        testing_prng_seed: Option<u64>,
        common_namespace: String,
        num_initial_validators: usize,
        num_proxies: usize,
        num_shards: usize,
        common_storage_config: InnerStorageConfig,
        cross_chain_config: CrossChainConfig,
        path_provider: PathProvider,
    ) -> Self {
        Self {
            network,
            directory,
            testing_prng_seed,
            next_client_id: 0,
            num_initial_validators,
            num_proxies,
            num_shards,
            validator_keys: BTreeMap::new(),
            running_validators: BTreeMap::new(),
            initialized_validator_storages: BTreeMap::new(),
            common_namespace,
            common_storage_config,
            cross_chain_config,
            path_provider,
        }
    }

    async fn command_for_binary(&self, name: &'static str) -> Result<Command> {
        println!("command_for_binary, directory={}", self.directory);
        let path = Path::new(&self.directory).join(name);
        println!("command_for_binary, step 1");
        let mut command = Command::new(path);
        println!("command_for_binary, step 2");
        command.current_dir(self.path_provider.path());
        println!("command_for_binary, step 3");
        Ok(command)
    }

    pub fn genesis_config(&self) -> Result<linera_client::config::GenesisConfig> {
        let path = self.path_provider.path();
        linera_service::util::read_json(path.join("genesis.json"))
    }

    fn shard_port(&self, validator: usize, shard: usize) -> usize {
        9000 + validator * self.num_shards + shard + 1
    }

    fn proxy_internal_port(&self, validator: usize, proxy_id: usize) -> usize {
        10000 + validator * self.num_proxies + proxy_id + 1
    }

    fn shard_metrics_port(&self, validator: usize, shard: usize) -> usize {
        11000 + validator * self.num_shards + shard + 1
    }

    fn proxy_metrics_port(&self, validator: usize, proxy_id: usize) -> usize {
        12000 + validator * self.num_proxies + proxy_id + 1
    }

    fn block_exporter_port(&self, validator: usize, exporter_id: usize) -> usize {
        12000 + validator * self.num_shards + exporter_id + 1
    }

    pub fn proxy_public_port(&self, validator: usize, proxy_id: usize) -> usize {
        FIRST_PUBLIC_PORT + validator * self.num_proxies + proxy_id + 1
    }

    pub fn first_public_port() -> usize {
        FIRST_PUBLIC_PORT + 1
    }

    fn block_exporter_metrics_port(exporter_id: usize) -> usize {
        FIRST_PUBLIC_PORT + exporter_id + 1
    }

    fn toml(network: Network) -> &'static str {
        match network {
            Network::Grpc => "{ Grpc = \"ClearText\" }",
            Network::Grpcs => "{ Grpc = \"Tls\" }",
            Network::Tcp => "{ Simple = \"Tcp\" }",
            Network::Udp => "{ Simple = \"Udp\" }",
        }
    }

    fn configuration_string(&self, server_number: usize) -> Result<String> {
        let n = server_number;
        let path = self
            .path_provider
            .path()
            .join(format!("validator_{n}.toml"));
        let port = self.proxy_public_port(n, 0);
        let external_protocol = Self::toml(self.network.external);
        let internal_protocol = Self::toml(self.network.internal);
        let external_host = self.network.external.localhost();
        let internal_host = self.network.internal.localhost();
        let mut content = format!(
            r#"
                server_config_path = "server_{n}.json"
                host = "{external_host}"
                port = {port}
                external_protocol = {external_protocol}
                internal_protocol = {internal_protocol}
            "#
        );

        for k in 0..self.num_proxies {
            let public_port = self.proxy_public_port(n, k);
            let internal_port = self.proxy_internal_port(n, k);
            let metrics_port = self.proxy_metrics_port(n, k);
            // In the local network, the validator ingress is
            // the proxy - so the `public_port` is the validator
            // port.
            content.push_str(&format!(
                r#"
                [[proxies]]
                host = "{internal_host}"
                public_port = {public_port}
                private_port = {internal_port}
                metrics_port = {metrics_port}
                "#
            ));
        }

        for k in 0..self.num_shards {
            let shard_port = self.shard_port(n, k);
            let shard_metrics_port = self.shard_metrics_port(n, k);
            content.push_str(&format!(
                r#"

                [[shards]]
                host = "{internal_host}"
                port = {shard_port}
                metrics_port = {shard_metrics_port}
                "#
            ));
        }

        fs_err::write(&path, content)?;
        path.into_os_string().into_string().map_err(|error| {
            anyhow!(
                "could not parse OS string into string: {}",
                error.to_string_lossy()
            )
        })
    }

    async fn generate_initial_validator_config(&mut self) -> Result<()> {
        let mut command = self.command_for_binary("linera-server").await?;
        command.arg("generate");
        if let Some(seed) = self.testing_prng_seed {
            command.arg("--testing-prng-seed").arg(seed.to_string());
            self.testing_prng_seed = Some(seed + 1);
        }
        command.arg("--validators");
        for i in 0..self.num_initial_validators {
            command.arg(&self.configuration_string(i)?);
        }
        let output = command
            .args(["--committee", "committee.json"])
            .spawn_and_wait_for_stdout()
            .await?;
        self.validator_keys = output
            .split_whitespace()
            .map(str::to_string)
            .map(|keys| keys.split(',').map(str::to_string).collect::<Vec<_>>())
            .enumerate()
            .map(|(i, keys)| {
                let validator_key = keys[0].to_string();
                let account_key = keys[1].to_string();
                (i, (validator_key, account_key))
            })
            .collect();
        Ok(())
    }

    async fn run_proxy(&mut self, validator: usize, proxy_id: usize) -> Result<Child> {
        use std::fs::File;
        let storage = self
            .initialized_validator_storages
            .get(&validator)
            .expect("initialized storage");
        let log_file = format!("LOG_proxy_{}_{}", validator, proxy_id);
        let log_file = File::create(log_file)?;
        let child = self
            .command_for_binary("linera-proxy")
            .await?
            .arg(format!("server_{}.json", validator))
            .args(["--storage", &storage.to_string()])
            .args(["--id", &proxy_id.to_string()])
            .stderr::<File>(log_file)
            .spawn_into()?;

        let port = self.proxy_public_port(validator, proxy_id);
        let nickname = format!("validator proxy {validator}");
        match self.network.external {
            Network::Grpc => {
                Self::ensure_grpc_server_has_started(&nickname, port, "http").await?;
                let nickname = format!("validator proxy {validator}");
                Self::ensure_grpc_server_has_started(&nickname, port, "http").await?;
            }
            Network::Grpcs => {
                let nickname = format!("validator proxy {validator}");
                Self::ensure_grpc_server_has_started(&nickname, port, "https").await?;
            }
            Network::Tcp => {
                Self::ensure_simple_server_has_started(&nickname, port, "tcp").await?;
            }
            Network::Udp => {
                Self::ensure_simple_server_has_started(&nickname, port, "udp").await?;
            }
        }
        Ok(child)
    }

    async fn run_exporter(&mut self, validator: usize, exporter_id: u32) -> Result<Child> {
        let config_path = format!("exporter_config_{validator}:{exporter_id}.toml");
        let storage = self
            .initialized_validator_storages
            .get(&validator)
            .expect("initialized storage");

        tracing::debug!(config=?config_path, storage=?storage.to_string(), "starting block exporter");

        let child = self
            .command_for_binary("linera-exporter")
            .await?
            .args(["--config-path", &config_path])
            .args(["--storage", &storage.to_string()])
            .spawn_into()?;

        match self.network.internal {
            Network::Grpc => {
                let port = self.block_exporter_port(validator, exporter_id as usize);
                let nickname = format!("block exporter {validator}:{exporter_id}");
                Self::ensure_grpc_server_has_started(&nickname, port, "http").await?;
            }
            Network::Grpcs => {
                let port = self.block_exporter_port(validator, exporter_id as usize);
                let nickname = format!("block exporter  {validator}:{exporter_id}");
                Self::ensure_grpc_server_has_started(&nickname, port, "https").await?;
            }
            Network::Tcp | Network::Udp => {
                unreachable!("Only allowed options are grpc and grpcs")
            }
        }

        tracing::info!("block exporter started {validator}:{exporter_id}");

        Ok(child)
    }

    pub async fn ensure_grpc_server_has_started(
        nickname: &str,
        port: usize,
        scheme: &str,
    ) -> Result<()> {
        let endpoint = match scheme {
            "http" => Endpoint::new(format!("http://localhost:{port}"))
                .context("endpoint should always parse")?,
            "https" => {
                use linera_rpc::CERT_PEM;
                let certificate = tonic::transport::Certificate::from_pem(CERT_PEM);
                let tls_config = ClientTlsConfig::new().ca_certificate(certificate);
                Endpoint::new(format!("https://localhost:{port}"))
                    .context("endpoint should always parse")?
                    .tls_config(tls_config)?
            }
            _ => bail!("Only supported scheme are http and https"),
        };
        let connection = endpoint.connect_lazy();
        let mut client = HealthClient::new(connection);
        linera_base::time::timer::sleep(Duration::from_millis(100)).await;
        for i in 0..10 {
            linera_base::time::timer::sleep(Duration::from_millis(i * 500)).await;
            let result = client.check(HealthCheckRequest::default()).await;
            if result.is_ok() && result.unwrap().get_ref().status() == ServingStatus::Serving {
                info!(?port, "Successfully started {nickname}");
                return Ok(());
            } else {
                warn!("Waiting for {nickname} to start");
            }
        }
        bail!("Failed to start {nickname}");
    }

    async fn ensure_simple_server_has_started(
        nickname: &str,
        port: usize,
        protocol: &str,
    ) -> Result<()> {
        use linera_core::node::ValidatorNode as _;

        let options = linera_rpc::NodeOptions {
            send_timeout: Duration::from_secs(5),
            recv_timeout: Duration::from_secs(5),
            retry_delay: Duration::from_secs(1),
            max_retries: 1,
        };
        let provider = linera_rpc::simple::SimpleNodeProvider::new(options);
        let address = format!("{protocol}:127.0.0.1:{port}");
        // All "simple" services (i.e. proxy and "server") are based on `RpcMessage` and
        // support `VersionInfoQuery`.
        let node = provider.make_node(&address)?;
        linera_base::time::timer::sleep(Duration::from_millis(100)).await;
        for i in 0..10 {
            linera_base::time::timer::sleep(Duration::from_millis(i * 500)).await;
            let result = node.get_version_info().await;
            if result.is_ok() {
                info!("Successfully started {nickname}");
                return Ok(());
            } else {
                warn!("Waiting for {nickname} to start");
            }
        }
        bail!("Failed to start {nickname}");
    }

    async fn initialize_storage(&mut self, validator: usize) -> Result<()> {
        let namespace = format!("{}_server_{}_db", self.common_namespace, validator);
        let inner_storage_config = self.common_storage_config.clone();
        let storage = StorageConfig {
            inner_storage_config,
            namespace,
        };
        let mut command = self.command_for_binary("linera").await?;
        if let Ok(var) = env::var(SERVER_ENV) {
            command.args(var.split_whitespace());
        }
        command.args(["storage", "initialize"]);
        command
            .args(["--storage", &storage.to_string()])
            .args(["--genesis", "genesis.json"])
            .spawn_and_wait_for_stdout()
            .await?;

        self.initialized_validator_storages
            .insert(validator, storage);
        Ok(())
    }

    async fn run_server(&mut self, validator: usize, shard: usize) -> Result<Child> {
        use std::fs::File;
        let mut storage = self
            .initialized_validator_storages
            .get(&validator)
            .expect("initialized storage")
            .clone();

        // For the storage backends with a local directory, make sure that we don't reuse
        // the same directory for all the shards.
        storage.maybe_append_shard_path(shard)?;

        let mut command = self.command_for_binary("linera-server").await?;
        if let Ok(var) = env::var(SERVER_ENV) {
            command.args(var.split_whitespace());
        }
        let log_file = format!("LOG_server_{}_{}", validator, shard);
        let log_file = File::create(log_file)?;
        command
            .arg("run")
            .args(["--storage", &storage.to_string()])
            .args(["--server", &format!("server_{}.json", validator)])
            .args(["--shard", &shard.to_string()])
            .args(self.cross_chain_config.to_args())
            .stderr::<File>(log_file);
        let child = command.spawn_into()?;

        let port = self.shard_port(validator, shard);
        let nickname = format!("validator server {validator}:{shard}");
        match self.network.internal {
            Network::Grpc => {
                Self::ensure_grpc_server_has_started(&nickname, port, "http").await?;
            }
            Network::Grpcs => {
                Self::ensure_grpc_server_has_started(&nickname, port, "https").await?;
            }
            Network::Tcp => {
                Self::ensure_simple_server_has_started(&nickname, port, "tcp").await?;
            }
            Network::Udp => {
                Self::ensure_simple_server_has_started(&nickname, port, "udp").await?;
            }
        }
        Ok(child)
    }

    async fn run(&mut self) -> Result<()> {
        for validator in 0..self.num_initial_validators {
            self.start_validator(validator).await?;
        }
        Ok(())
    }

    /// Start a validator.
    pub async fn start_validator(&mut self, index: usize) -> Result<()> {
        self.initialize_storage(index).await?;
        self.restart_validator(index).await
    }

    /// Restart a validator. This is similar to `start_validator` except that the
    /// database was already initialized once.
    pub async fn restart_validator(&mut self, index: usize) -> Result<()> {
        let mut validator = Validator::new();
        for k in 0..self.num_proxies {
            let proxy = self.run_proxy(index, k).await?;
            validator.add_proxy(proxy);
        }
        for shard in 0..self.num_shards {
            let server = self.run_server(index, shard).await?;
            validator.add_server(server);
        }

        self.running_validators.insert(index, validator);
        Ok(())
    }

    /// Terminates all the processes of a given validator.
    pub async fn stop_validator(&mut self, index: usize) -> Result<()> {
        if let Some(mut validator) = self.running_validators.remove(&index) {
            if let Err(error) = validator.terminate().await {
                error!("Failed to stop validator {index}: {error}");
                return Err(error);
            }
        }
        Ok(())
    }

    /// Returns a [`linera_rpc::Client`] to interact directly with a `validator`.
    pub fn validator_client(&mut self, validator: usize) -> Result<linera_rpc::Client> {
        let node_provider = linera_rpc::NodeProvider::new(linera_rpc::NodeOptions {
            send_timeout: Duration::from_secs(1),
            recv_timeout: Duration::from_secs(1),
            retry_delay: Duration::ZERO,
            max_retries: 0,
        });

        Ok(node_provider.make_node(&self.validator_address(validator))?)
    }

    /// Returns the address to connect to a validator's proxy.
    /// In local networks, the zeroth proxy _is_ the validator ingress.
    pub fn validator_address(&self, validator: usize) -> String {
        let port = self.proxy_public_port(validator, 0);
        let schema = self.network.external.schema();

        format!("{schema}:localhost:{port}")
    }
}

impl SpecifiedLocalNet {
    /// Returns the validating key and an account key of the validator.
    pub fn validator_keys(&self, validator: usize) -> Option<&(String, String)> {
        self.validator_keys.get(&validator)
    }

    pub async fn generate_validator_config(&mut self, validator: usize) -> Result<()> {
        let stdout = self
            .command_for_binary("linera-server")
            .await?
            .arg("generate")
            .arg("--validators")
            .arg(&self.configuration_string(validator)?)
            .spawn_and_wait_for_stdout()
            .await?;
        let keys = stdout
            .trim()
            .split(',')
            .map(str::to_string)
            .collect::<Vec<_>>();
        self.validator_keys
            .insert(validator, (keys[0].clone(), keys[1].clone()));
        Ok(())
    }

    pub async fn terminate_server(&mut self, validator: usize, shard: usize) -> Result<()> {
        self.running_validators
            .get_mut(&validator)
            .context("server not found")?
            .terminate_server(shard)
            .await?;
        Ok(())
    }

    pub fn remove_validator(&mut self, validator: usize) -> Result<()> {
        self.running_validators
            .remove(&validator)
            .context("validator not found")?;
        Ok(())
    }

    pub async fn start_server(&mut self, validator: usize, shard: usize) -> Result<()> {
        let server = self.run_server(validator, shard).await?;
        self.running_validators
            .get_mut(&validator)
            .context("could not find validator")?
            .add_server(server);
        Ok(())
    }
}
