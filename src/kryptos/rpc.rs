//! Client for Kryptos's authenticated, line-oriented node control service.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const MAX_REPLY_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(crate) struct Credentials {
    pub endpoint: String,
    pub token: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("endpoint", &self.endpoint)
            .field("token", &"[redacted]")
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct Client {
    endpoint: SocketAddr,
    token: String,
}

pub(crate) fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let address = endpoint
        .trim()
        .parse::<SocketAddr>()
        .map_err(|_| "Use a local control address such as 127.0.0.1:18440".to_owned())?;
    if !address.is_ipv4() || !address.ip().is_loopback() || address.port() == 0 {
        return Err("The GUI accepts only an IPv4 loopback control address".into());
    }
    Ok(())
}

impl Client {
    pub(crate) fn new(credentials: &Credentials) -> Result<Self, String> {
        validate_endpoint(&credentials.endpoint)?;
        let token = credentials.token.trim();
        if token.is_empty() || token.len() > 128 || token.contains(char::is_whitespace) {
            return Err("Enter the node token from its rpc.token file".into());
        }
        let endpoint = credentials
            .endpoint
            .trim()
            .parse()
            .map_err(|_| "The local control address is invalid".to_owned())?;
        Ok(Self {
            endpoint,
            token: token.to_owned(),
        })
    }

    pub(crate) async fn call(&self, command: &str) -> Result<String, String> {
        if command.is_empty() || command.contains('\n') || command.contains('\r') {
            return Err("Invalid control command".into());
        }
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            let stream = TcpStream::connect(self.endpoint)
                .await
                .map_err(|_| "Could not connect to the local Kryptos control socket".to_owned())?;
            let (read, mut write) = stream.into_split();
            let mut reader = BufReader::new(read);

            write
                .write_all(format!("auth {}\n", self.token).as_bytes())
                .await
                .map_err(|_| "Could not authenticate with the node".to_owned())?;
            let auth = read_bounded_line(&mut reader)
                .await
                .map_err(|_| "Could not read the node's authentication reply".to_owned())?;
            if auth != "ok" {
                return Err("Node authentication failed; check the token".into());
            }

            write
                .write_all(format!("{command}\n").as_bytes())
                .await
                .map_err(|_| "Could not send a command to the node".to_owned())?;
            let reply = read_bounded_line(&mut reader)
                .await
                .map_err(|_| "Could not read the node's control reply".to_owned())?;
            if reply == "err unauthorized" {
                return Err("Node authentication failed; check the token".into());
            }
            if reply.starts_with("err ") {
                return Err("The node rejected the control command".into());
            }
            if reply == "ok" {
                return Ok(String::new());
            }
            reply
                .strip_prefix("ok ")
                .map(|value| value.trim_start().to_owned())
                .ok_or_else(|| "The node returned an invalid control reply".into())
        })
        .await
        .map_err(|_| "Local control request timed out after 10 seconds".to_owned())?
    }
}

async fn read_bounded_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let (chunk, consumed, complete) = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "control connection closed before reply",
                ));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let consumed = newline.map_or(available.len(), |index| index + 1);
            if consumed > MAX_REPLY_BYTES.saturating_sub(bytes.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "control reply exceeds size limit",
                ));
            }
            (available[..consumed].to_vec(), consumed, newline.is_some())
        };
        bytes.extend_from_slice(&chunk);
        reader.consume(consumed);
        if complete {
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
            }
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return String::from_utf8(bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct NodeSnapshot {
    pub height: u32,
    pub hash: String,
    pub peers: usize,
    pub mempool: usize,
    pub mined: usize,
    pub mined_in_chain: usize,
    pub syncing: bool,
}

pub(crate) async fn fetch_node(credentials: Credentials) -> Result<NodeSnapshot, String> {
    let reply = Client::new(&credentials)?.call("status").await?;
    let fields: HashMap<_, _> = reply
        .split_whitespace()
        .filter_map(|field| field.split_once('='))
        .collect();
    let number = |name: &str| {
        fields
            .get(name)
            .ok_or_else(|| "The node returned an incomplete status".to_owned())?
            .parse::<usize>()
            .map_err(|_| "The node returned an invalid status".to_owned())
    };
    let height = fields
        .get("height")
        .ok_or_else(|| "The node returned an incomplete status".to_owned())?
        .parse::<u32>()
        .map_err(|_| "The node returned an invalid status".to_owned())?;
    let hash = fields
        .get("hash")
        .filter(|hash| is_hash(hash))
        .ok_or_else(|| "The node returned an invalid tip hash".to_owned())?
        .to_string();
    let syncing = fields
        .get("syncing")
        .ok_or_else(|| "The node returned an incomplete status".to_owned())?
        .parse::<bool>()
        .map_err(|_| "The node returned an invalid sync state".to_owned())?;
    Ok(NodeSnapshot {
        height,
        hash,
        peers: number("peers")?,
        mempool: number("mempool")?,
        mined: number("mined")?,
        mined_in_chain: number("in_chain")?,
        syncing,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LookupKind {
    Block,
    Transaction,
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) async fn lookup(
    credentials: Credentials,
    kind: LookupKind,
    query: String,
) -> Result<Value, String> {
    let client = Client::new(&credentials)?;
    let query = query.trim();
    match kind {
        LookupKind::Block => {
            let height = query
                .parse::<u32>()
                .ok()
                .filter(|_| !query.is_empty() && query.bytes().all(|byte| byte.is_ascii_digit()))
                .ok_or_else(|| "Enter a block height from 0 to 4294967295".to_owned())?;
            let hash = client.call(&format!("hash {height}")).await?;
            if !is_hash(&hash) {
                return Err("The node returned an invalid block hash".into());
            }
            Ok(json!({"type": "block", "height": height, "hash": hash}))
        }
        LookupKind::Transaction => {
            if !is_hash(query) {
                return Err("Enter a 64-character transaction ID".into());
            }
            let status = client
                .call(&format!("txstatus {}", query.to_ascii_lowercase()))
                .await?;
            let mut result = json!({"type": "transaction", "txid": query.to_ascii_lowercase()});
            match status.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["unknown"] => result["status"] = json!("unknown"),
                ["pooled"] => result["status"] = json!("pooled"),
                ["confirmed", height, index] => {
                    let height = height.parse::<u32>().map_err(|_| {
                        "The node returned an invalid transaction status".to_owned()
                    })?;
                    let index = index.parse::<u32>().map_err(|_| {
                        "The node returned an invalid transaction status".to_owned()
                    })?;
                    result["status"] = json!("confirmed");
                    result["height"] = json!(height);
                    result["index"] = json!(index);
                }
                _ => return Err("The node returned an invalid transaction status".into()),
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use protocol::block::BlockHash;
    use runtime::node::{Event, Request, Response, TxStatus};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    const FIXTURE_TIMEOUT: Duration = Duration::from_secs(15);

    #[tokio::test]
    async fn external_client_interoperates_with_the_core_control_service() {
        tokio::time::timeout(FIXTURE_TIMEOUT, async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let credentials = Credentials {
                endpoint: listener.local_addr().unwrap().to_string(),
                token: "fixture-token".into(),
            };
            let token = runtime::rpc::Token::new(&credentials.token).unwrap();
            let hash = BlockHash::from_bytes(*b"0123456789abcdefghijklmnopqrstuv");
            let expected_hash = crypto::encoding::to_hex(hash.as_bytes());
            let (events, mut receiver) = mpsc::channel(1);

            // Exercise the real core authentication, command parser, and reply
            // formatter without starting storage, proof generation, or Tor.
            let server = tokio::spawn(async move {
                for _ in 0..7 {
                    let (stream, _) = listener.accept().await.unwrap();
                    runtime::rpc::serve_stream(stream, events.clone(), token.clone()).await;
                }
            });
            let responder = tokio::spawn(async move {
                let mut requests = 0;
                while let Some(event) = receiver.recv().await {
                    let Event::Rpc { request, reply } = event else {
                        panic!("unexpected event in control fixture");
                    };
                    requests += 1;
                    let response = match request {
                        Request::Status => Response::Status {
                            height: 12,
                            hash,
                            peers: 3,
                            mempool: 2,
                            mined: 5,
                            mined_in_chain: 4,
                            syncing: true,
                        },
                        Request::Hash(12) => Response::Hash(Some(hash)),
                        Request::Hash(13) => Response::Hash(None),
                        Request::TransactionStatus(txid) => {
                            let status = match txid.as_bytes() {
                                bytes if bytes == &[0x00; 32] => TxStatus::Unknown,
                                bytes if bytes == &[0x11; 32] => TxStatus::Pooled,
                                bytes if bytes == &[0xab; 32] => TxStatus::Confirmed {
                                    height: 12,
                                    index: 2,
                                },
                                _ => panic!("unexpected transaction ID in control fixture"),
                            };
                            Response::TransactionStatus(status)
                        }
                        _ => panic!("unexpected request in control fixture"),
                    };
                    reply.send(response).unwrap();
                }
                requests
            });

            let snapshot = fetch_node(credentials.clone()).await.unwrap();
            assert_eq!(snapshot.height, 12);
            assert_eq!(snapshot.hash, expected_hash);
            assert_eq!(snapshot.peers, 3);
            assert_eq!(snapshot.mempool, 2);
            assert_eq!(snapshot.mined, 5);
            assert_eq!(snapshot.mined_in_chain, 4);
            assert!(snapshot.syncing);

            let block = lookup(credentials.clone(), LookupKind::Block, " 12 ".into())
                .await
                .unwrap();
            assert_eq!(
                block,
                json!({"type": "block", "height": 12, "hash": expected_hash})
            );
            assert!(
                lookup(credentials.clone(), LookupKind::Block, "13".into())
                    .await
                    .unwrap_err()
                    .contains("rejected")
            );
            for (query, status) in [("00", "unknown"), ("11", "pooled"), ("AB", "confirmed")] {
                let transaction = lookup(
                    credentials.clone(),
                    LookupKind::Transaction,
                    query.repeat(32),
                )
                .await
                .unwrap();
                assert_eq!(transaction["txid"], query.to_ascii_lowercase().repeat(32));
                assert_eq!(transaction["status"], status);
                if status == "confirmed" {
                    assert_eq!(transaction["height"], 12);
                    assert_eq!(transaction["index"], 2);
                }
            }
            let unauthorized = Credentials {
                token: "wrong-token".into(),
                ..credentials
            };
            assert!(
                fetch_node(unauthorized)
                    .await
                    .unwrap_err()
                    .contains("authentication failed")
            );
            server.await.unwrap();
            // A rejected token must never dispatch a request to the node.
            assert_eq!(responder.await.unwrap(), 6);
        })
        .await
        .expect("control service fixture timed out");
    }

    #[tokio::test]
    async fn malformed_success_prefix_is_rejected() {
        tokio::time::timeout(FIXTURE_TIMEOUT, async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let credentials = Credentials {
                endpoint: listener.local_addr().unwrap().to_string(),
                token: "fixture-token".into(),
            };
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                let mut line = String::new();
                stream.read_line(&mut line).await.unwrap();
                assert_eq!(line, "auth fixture-token\n");
                stream.get_mut().write_all(b"ok\n").await.unwrap();
                line.clear();
                stream.read_line(&mut line).await.unwrap();
                assert_eq!(line, "status\n");
                let reply = format!(
                    "okay height=12 hash={} peers=3 mempool=2 mined=5 in_chain=4 syncing=false\n",
                    "00".repeat(32)
                );
                stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
            });
            assert!(
                fetch_node(credentials)
                    .await
                    .unwrap_err()
                    .contains("invalid control reply")
            );
            server.await.unwrap();
        })
        .await
        .expect("malformed reply fixture timed out");
    }
}
