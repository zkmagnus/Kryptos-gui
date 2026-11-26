//! Owns an embedded node on a runtime independent of the desktop event loop.
//!
//! The actor keeps the node alive across cancelled UI tasks and serializes its
//! lifecycle. Closing the command channel also completes an orderly shutdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crypto::encoding::{from_hex, to_hex};
use protocol::transaction::TxId;
use runtime::config::{Config, Network};
use runtime::node::{Handle, Request, Response, TxStatus};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use super::rpc::{LookupKind, NodeSnapshot};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
// Building the proof verification key on a cold start can take longer than a
// normal control request, particularly in an unoptimized development build.
const START_TIMEOUT: Duration = Duration::from_secs(180);
const NODE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub(crate) struct NodeOptions {
    pub network: String,
    pub datadir: PathBuf,
    pub peers: Vec<String>,
}

impl NodeOptions {
    fn config(self) -> Result<Config, String> {
        let network = self.network.parse::<Network>()?;
        if self.datadir.as_os_str().is_empty() {
            return Err("Choose a directory for the node's data.".into());
        }
        let config = Config {
            network,
            datadir: self.datadir,
            cleanup_datadir: false,
            offline: false,
            p2p: true,
            connect: Vec::new(),
            addnode: self.peers,
            no_seeds: false,
            max_outbound: runtime::config::OUTBOUND_TARGET,
            tor: runtime::config::TorConfig::default(),
            mine_to: None,
            mining_threads: 1,
            bootstrap_mining: false,
            mining_min_peers: 1,
            mining_ready_secs: 5,
            rpc: None,
            rpc_token: None,
            block_notify: None,
            max_inbound: runtime::config::DEFAULT_MAX_INBOUND,
        };
        config.validate().map_err(|error| error.to_string())?;
        Ok(config)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EmbeddedSnapshot {
    pub node: NodeSnapshot,
    pub tor_status: String,
    pub onion_address: Option<String>,
}

type Reply<T> = oneshot::Sender<Result<T, String>>;

enum Command {
    Start(Box<Config>, u64, Reply<()>),
    Stop(Reply<()>),
    Snapshot(Reply<EmbeddedSnapshot>),
    Lookup(LookupKind, String, Reply<Value>),
}

#[derive(Clone)]
pub(crate) struct EmbeddedNode {
    commands: mpsc::Sender<Command>,
    initialization_error: Option<String>,
    lifecycle: Arc<AtomicU64>,
}

impl EmbeddedNode {
    pub fn new() -> Self {
        let (commands, receiver) = mpsc::channel(16);
        let lifecycle = Arc::new(AtomicU64::new(0));
        let actor_lifecycle = Arc::clone(&lifecycle);
        let initialization_error = std::thread::Builder::new()
            .name("kryptos-node".into())
            .spawn(move || {
                let Ok(executor) = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                else {
                    return;
                };
                executor.block_on(run(receiver, actor_lifecycle));
            })
            .err()
            .map(|error| format!("Could not start the node service: {error}"));
        Self {
            commands,
            initialization_error,
            lifecycle,
        }
    }

    pub fn start(
        &self,
        options: NodeOptions,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        // Capture before the future is polled. Closing the UI can cancel even
        // a startup task that the executor has not begun to poll yet.
        let generation = self.lifecycle.load(Ordering::SeqCst);
        let service = self.clone();
        async move {
            let config = options.config()?;
            service.start_config(config, generation).await
        }
    }

    pub fn stop(&self) -> impl Future<Output = Result<(), String>> + Send + 'static {
        self.lifecycle.fetch_add(1, Ordering::SeqCst);
        let service = self.clone();
        async move {
            if service.initialization_error.is_some() || service.commands.is_closed() {
                // No actor remains that can own a running node.
                return Ok(());
            }
            service
                .request(Command::Stop, "shutdown", COMMAND_TIMEOUT)
                .await
        }
    }

    pub async fn snapshot(&self) -> Result<EmbeddedSnapshot, String> {
        self.request(Command::Snapshot, "status request", COMMAND_TIMEOUT)
            .await
    }

    pub async fn lookup(&self, kind: LookupKind, query: String) -> Result<Value, String> {
        self.request(
            |reply| Command::Lookup(kind, query, reply),
            "search",
            COMMAND_TIMEOUT,
        )
        .await
    }

    async fn start_config(&self, config: Config, generation: u64) -> Result<(), String> {
        if self.lifecycle.load(Ordering::SeqCst) != generation {
            return Err("Node startup was cancelled.".into());
        }
        self.request(
            |reply| Command::Start(Box::new(config), generation, reply),
            "startup",
            START_TIMEOUT,
        )
        .await
    }

    async fn request<T>(
        &self,
        command: impl FnOnce(Reply<T>) -> Command,
        action: &str,
        timeout: Duration,
    ) -> Result<T, String> {
        if let Some(error) = &self.initialization_error {
            return Err(error.clone());
        }
        let (reply, response) = oneshot::channel();
        tokio::time::timeout(timeout, async {
            self.commands
                .send(command(reply))
                .await
                .map_err(|_| "The embedded node service is unavailable.".to_string())?;
            response
                .await
                .map_err(|_| "The embedded node service stopped unexpectedly.".to_string())?
        })
        .await
        .map_err(|_| {
            format!(
                "Node {action} is taking longer than expected. The node service still owns the node; wait or choose Stop node."
            )
        })?
    }

    #[cfg(test)]
    fn start_offline(
        &self,
        config: Config,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        assert!(config.offline, "tests must never start external networking");
        let generation = self.lifecycle.load(Ordering::SeqCst);
        let service = self.clone();
        async move { service.start_config(config, generation).await }
    }
}

async fn run(mut commands: mpsc::Receiver<Command>, lifecycle: Arc<AtomicU64>) {
    let mut node: Option<Handle> = None;
    while let Some(command) = commands.recv().await {
        match command {
            Command::Start(config, generation, reply) => {
                if reply.is_closed() {
                    continue;
                }
                if lifecycle.load(Ordering::SeqCst) != generation {
                    let _ = reply.send(Err("Node startup was cancelled.".into()));
                    continue;
                }
                if node.is_some() {
                    let _ =
                        reply.send(Err("Stop the current node before starting another.".into()));
                    continue;
                }
                let result = match start_node(*config).await {
                    Ok(handle) => {
                        if lifecycle.load(Ordering::SeqCst) != generation {
                            handle.shutdown().await;
                            Err("Node startup was cancelled.".into())
                        } else {
                            // A timed-out or cancelled UI request must not drop
                            // a running Handle: Stop and channel closure own it.
                            node = Some(handle);
                            Ok(())
                        }
                    }
                    Err(error) => Err(error),
                };
                let _ = reply.send(result);
            }
            Command::Stop(reply) => {
                if let Some(handle) = node.take() {
                    // Never wrap this future in a timeout. It owns the store,
                    // Tor tasks and miner until shutdown finishes, even when
                    // the UI stops waiting for its response.
                    handle.shutdown().await;
                }
                let _ = reply.send(Ok(()));
            }
            Command::Snapshot(reply) => {
                let result = match &node {
                    Some(handle) => snapshot(handle).await,
                    None => Err("The embedded node is stopped.".into()),
                };
                let _ = reply.send(result);
            }
            Command::Lookup(kind, query, reply) => {
                let result = match &node {
                    Some(handle) => lookup(handle, kind, query.trim()).await,
                    None => Err("Start the embedded node before searching.".into()),
                };
                let _ = reply.send(result);
            }
        }
    }
    if let Some(handle) = node {
        handle.shutdown().await;
    }
}

async fn start_node(config: Config) -> Result<Handle, String> {
    config.validate().map_err(|error| error.to_string())?;
    runtime::paths::private_directory(&config.datadir).map_err(|error| error.to_string())?;
    runtime::node::spawn(config)
        .await
        .map_err(|error| error.to_string())
}

async fn call(handle: &Handle, request: Request) -> Result<Response, String> {
    let response = tokio::time::timeout(NODE_REQUEST_TIMEOUT, handle.request(request))
        .await
        .map_err(|_| "The embedded node did not answer within 10 seconds.".to_string())?
        .map_err(|error| error.to_string())?;
    match response {
        Response::Failed(reason) => Err(reason),
        response => Ok(response),
    }
}

async fn snapshot(handle: &Handle) -> Result<EmbeddedSnapshot, String> {
    let Response::Status {
        height,
        hash,
        peers,
        mempool,
        mined,
        mined_in_chain,
        syncing,
    } = call(handle, Request::Status).await?
    else {
        return Err("The embedded node returned an invalid status.".into());
    };
    let (tor_status, onion_address) = match &handle.tor_status {
        None => ("Offline", None),
        Some(status) if status.has_changed().is_err() => ("Tor service stopped", None),
        Some(status) => {
            let state = status.borrow();
            if !state.client_ready {
                ("Connecting to Tor", None)
            } else if let Some(onion) = state.onion {
                (
                    "Ready",
                    Some(onion.endpoint(runtime::config::DEFAULT_P2P_PORT)),
                )
            } else {
                ("Publishing onion service", None)
            }
        }
    };
    Ok(EmbeddedSnapshot {
        node: NodeSnapshot {
            height,
            hash: to_hex(hash.as_bytes()),
            peers,
            mempool,
            mined,
            mined_in_chain,
            syncing,
        },
        tor_status: tor_status.into(),
        onion_address,
    })
}

async fn lookup(handle: &Handle, kind: LookupKind, query: &str) -> Result<Value, String> {
    match kind {
        LookupKind::Block => {
            let height = query
                .parse::<u32>()
                .ok()
                .filter(|_| !query.is_empty() && query.bytes().all(|byte| byte.is_ascii_digit()))
                .ok_or_else(|| "Enter a block height from 0 to 4294967295".to_string())?;
            match call(handle, Request::Hash(height)).await? {
                Response::Hash(Some(hash)) => {
                    Ok(json!({"type": "block", "height": height, "hash": to_hex(hash.as_bytes())}))
                }
                Response::Hash(None) => Err("There is no block at that height.".into()),
                _ => Err("The embedded node returned an invalid block hash.".into()),
            }
        }
        LookupKind::Transaction => {
            if query.len() != 64 {
                return Err("Enter a 64-character transaction ID".into());
            }
            let bytes: [u8; 32] = from_hex(query)
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or_else(|| "Enter a 64-character transaction ID".to_string())?;
            let mut result = json!({"type": "transaction", "txid": query.to_ascii_lowercase()});
            match call(handle, Request::TransactionStatus(TxId::from_bytes(bytes))).await? {
                Response::TransactionStatus(TxStatus::Unknown) => {
                    result["status"] = json!("unknown")
                }
                Response::TransactionStatus(TxStatus::Pooled) => result["status"] = json!("pooled"),
                Response::TransactionStatus(TxStatus::Confirmed { height, index }) => {
                    result["status"] = json!("confirmed");
                    result["height"] = json!(height);
                    result["index"] = json!(index);
                }
                _ => return Err("The embedded node returned an invalid transaction status.".into()),
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn config() -> (Self, Config) {
            let config = Config {
                cleanup_datadir: false,
                ..Config::test()
            };
            (Self(config.datadir.clone()), config)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn offline_node_stops_and_reopens_its_persistent_chain() {
        let (_directory, config) = TestDirectory::config();
        let embedded = EmbeddedNode::new();
        embedded.start_offline(config.clone()).await.unwrap();
        let initial = embedded.snapshot().await.unwrap();
        assert_eq!(initial.node.height, 0);
        assert_eq!(initial.node.peers, 0);
        assert_eq!(initial.tor_status, "Offline");
        assert!(initial.onion_address.is_none());
        let block = embedded
            .lookup(LookupKind::Block, "0".into())
            .await
            .unwrap();
        assert_eq!(block["hash"], initial.node.hash);
        assert!(
            embedded
                .lookup(LookupKind::Block, "1".into())
                .await
                .is_err()
        );
        let transaction = embedded
            .lookup(LookupKind::Transaction, "00".repeat(32))
            .await
            .unwrap();
        assert_eq!(transaction["status"], "unknown");
        assert!(embedded.start_offline(config.clone()).await.is_err());
        assert_eq!(
            embedded.snapshot().await.unwrap().node.hash,
            initial.node.hash
        );
        embedded.stop().await.unwrap();
        assert!(embedded.snapshot().await.is_err());
        assert!(config.datadir.is_dir());
        embedded.start_offline(config).await.unwrap();
        assert_eq!(
            embedded.snapshot().await.unwrap().node.hash,
            initial.node.hash
        );
        embedded.stop().await.unwrap();
    }

    #[tokio::test]
    async fn an_occupied_store_is_rejected_and_released_after_stop() {
        let (_directory, config) = TestDirectory::config();
        let first = EmbeddedNode::new();
        let second = EmbeddedNode::new();
        first.start_offline(config.clone()).await.unwrap();
        assert!(second.start_offline(config.clone()).await.is_err());
        assert_eq!(first.snapshot().await.unwrap().node.height, 0);
        first.stop().await.unwrap();
        second.start_offline(config).await.unwrap();
        assert_eq!(second.snapshot().await.unwrap().node.height, 0);
        second.stop().await.unwrap();
    }

    #[tokio::test]
    async fn dropping_all_clients_shuts_down_the_owned_node() {
        let (_directory, config) = TestDirectory::config();
        let first = EmbeddedNode::new();
        first.start_offline(config.clone()).await.unwrap();
        drop(first);
        let replacement = EmbeddedNode::new();
        tokio::time::timeout(START_TIMEOUT + Duration::from_secs(10), async {
            loop {
                if replacement.start_offline(config.clone()).await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("dropping the command channel retained the node's database");
        replacement.stop().await.unwrap();
    }

    #[tokio::test]
    async fn stop_cancels_an_unpolled_startup_future() {
        let (directory, config) = TestDirectory::config();
        let embedded = EmbeddedNode::new();
        let startup = embedded.start_offline(config);
        embedded.stop().await.unwrap();
        assert!(startup.await.unwrap_err().contains("cancelled"));
        assert!(!directory.0.exists());
        assert!(embedded.snapshot().await.is_err());
    }

    #[tokio::test]
    async fn invalid_configuration_is_rejected_before_opening_the_store() {
        let (directory, _) = TestDirectory::config();
        let embedded = EmbeddedNode::new();
        for (network, peers) in [
            ("other", Vec::new()),
            ("test", vec!["127.0.0.1:19000".into()]),
            ("main", vec!["abc.onion:19000".into()]),
        ] {
            let result = embedded
                .start(NodeOptions {
                    network: network.into(),
                    datadir: directory.0.clone(),
                    peers,
                })
                .await;
            assert!(result.is_err());
        }
        assert!(!directory.0.exists());
        assert!(embedded.snapshot().await.is_err());
    }
}
