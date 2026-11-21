//! Authenticated access to the existing Rhombus node and wallet HTTP interfaces.
//!
//! Only the local 127.0.0.1 bind is accepted. Rhombus itself owns Tor transport;
//! this desktop client never turns a node or wallet credential into a remote request.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use reqwest::Url;
use reqwest::header::HeaderValue;
use serde_json::Value;
use serde_json::json;

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
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
    http: reqwest::Client,
    endpoint: Url,
    authorization: HeaderValue,
    next_id: Arc<AtomicU64>,
}

fn endpoint_url(endpoint: &str) -> Result<Url, String> {
    let error = || "Use a local RPC address such as http://127.0.0.1:18443/".to_owned();
    let endpoint = endpoint.trim();
    let rest = endpoint
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(error)?;
    let port = rest.strip_suffix('/').unwrap_or(rest);
    if port.is_empty()
        || !port.bytes().all(|c| c.is_ascii_digit())
        || port.parse::<u16>().ok().filter(|p| *p != 0).is_none()
    {
        return Err(error());
    }
    let url = Url::parse(endpoint).map_err(|_| error())?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error());
    }
    Ok(url)
}

pub(crate) fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    endpoint_url(endpoint).map(|_| ())
}

impl Client {
    pub(crate) fn new(credentials: &Credentials) -> Result<Self, String> {
        let endpoint = endpoint_url(&credentials.endpoint)?;
        let token = credentials.token.trim();
        if token.is_empty() || !token.bytes().all(|c| c.is_ascii_graphic()) {
            return Err("Enter the RPC token from the corresponding daemon's token file".into());
        }
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "The RPC token contains invalid characters".to_owned())?;
        authorization.set_sensitive(true);
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| "Could not initialize the local RPC client".to_owned())?;
        Ok(Self {
            http,
            endpoint,
            authorization,
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut response = self
            .http
            .post(self.endpoint.clone())
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .json(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .send()
            .await
            .map_err(transport_error)?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => "RPC authentication failed; check the daemon's token".into(),
                status => format!("Local RPC returned HTTP {status}"),
            });
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err("RPC response exceeds the 8 MiB display limit".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err("RPC response exceeds the 8 MiB display limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "The daemon returned invalid JSON".to_owned())?;
        decode_response(body, id)
    }
}

fn transport_error(error: reqwest::Error) -> String {
    // Never include a URL, response body, request header, or server-provided
    // diagnostic in an error that can be shown by the UI or logged.
    if error.is_timeout() {
        "Local RPC timed out after 10 seconds".into()
    } else if error.is_connect() {
        "Could not connect to local RPC; check the daemon and HTTP port".into()
    } else {
        "The local RPC connection failed".into()
    }
}

fn decode_response(body: Value, id: u64) -> Result<Value, String> {
    if body.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || body.get("id").and_then(Value::as_u64) != Some(id)
    {
        return Err("The daemon returned an invalid JSON-RPC response".into());
    }
    if let Some(error) = body.get("error") {
        if body.get("result").is_some() || error.get("code").and_then(Value::as_i64).is_none() {
            return Err("The daemon returned an invalid JSON-RPC error".into());
        }
        // A remote message could reflect a token. Display recognized codes only.
        let code = error["code"].as_i64().unwrap_or_default();
        return Err(match code {
            -32001 => "RPC authentication failed; check the daemon's token".into(),
            -32601 => "This method is unavailable on the selected daemon".into(),
            -32602 => "The daemon rejected the lookup parameters".into(),
            -1 => "The requested block or transaction was not found".into(),
            -3 => "The Kryptos node has stopped".into(),
            _ => format!("Kryptos RPC failed (code {code})"),
        });
    }
    body.get("result")
        .cloned()
        .ok_or_else(|| "The daemon returned no JSON-RPC result".to_owned())
}

#[derive(Clone, Debug)]
pub(crate) struct NodeSnapshot {
    pub chain: Value,
    pub metrics: Value,
    pub network: Value,
    pub mining: Value,
    pub peers: Vec<Value>,
    pub mempool: Vec<String>,
    pub mempool_info: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct WalletSnapshot {
    pub info: Value,
    pub balance: Value,
}

type FieldValidator = fn(&Value) -> bool;

fn require_fields(value: &Value, fields: &[(&str, FieldValidator)]) -> Result<(), String> {
    if !value.is_object()
        || fields
            .iter()
            .any(|(name, valid)| !value.get(name).is_some_and(valid))
    {
        return Err("The daemon returned an unexpected data format".into());
    }
    Ok(())
}

pub(crate) async fn fetch_node(credentials: Credentials) -> Result<NodeSnapshot, String> {
    let client = Client::new(&credentials)?;
    let (chain, metrics, network, mining, peers, mempool, mempool_info) = tokio::try_join!(
        client.call("getblockchaininfo", json!([])),
        client.call("getmetrics", json!([])),
        client.call("getnetworkinfo", json!([])),
        client.call("getmininginfo", json!([])),
        client.call("getpeerinfo", json!([])),
        client.call("getrawmempool", json!([])),
        client.call("getmempoolinfo", json!([])),
    )?;
    require_fields(
        &chain,
        &[
            ("height", Value::is_u64),
            ("network", Value::is_string),
            ("best_block_hash", Value::is_string),
            ("syncing", Value::is_boolean),
        ],
    )?;
    require_fields(
        &metrics,
        &[
            ("height", Value::is_u64),
            ("peers_inbound", Value::is_u64),
            ("peers_outbound", Value::is_u64),
            ("uptime_seconds", Value::is_u64),
            ("mining_ready", Value::is_boolean),
        ],
    )?;
    require_fields(
        &network,
        &[
            ("network", Value::is_string),
            ("connections", Value::is_u64),
        ],
    )?;
    require_fields(
        &mining,
        &[
            ("height", Value::is_u64),
            ("subsidy", Value::is_string),
            ("block_interval", Value::is_u64),
        ],
    )?;
    require_fields(
        &mempool_info,
        &[("size", Value::is_u64), ("capacity", Value::is_u64)],
    )?;
    let peers = peers
        .as_array()
        .ok_or_else(|| "The daemon returned an invalid peer list".to_owned())?
        .clone();
    for peer in &peers {
        require_fields(
            peer,
            &[
                ("direction", Value::is_string),
                ("ready", Value::is_boolean),
            ],
        )?;
    }
    let mempool = mempool
        .as_array()
        .ok_or_else(|| "The daemon returned an invalid mempool".to_owned())?
        .iter()
        .map(|txid| {
            txid.as_str()
                .filter(|txid| is_hash(txid))
                .map(str::to_owned)
                .ok_or_else(|| "The daemon returned an invalid transaction ID".to_owned())
        })
        .collect::<Result<_, _>>()?;
    Ok(NodeSnapshot {
        chain,
        metrics,
        network,
        mining,
        peers,
        mempool,
        mempool_info,
    })
}

pub(crate) async fn fetch_wallet(credentials: Credentials) -> Result<WalletSnapshot, String> {
    let client = Client::new(&credentials)?;
    let (info, balance) = tokio::try_join!(
        client.call("getwalletinfo", json!([])),
        client.call("getbalance", json!([])),
    )?;
    require_fields(
        &info,
        &[
            ("network", Value::is_string),
            ("synced", Value::is_boolean),
            ("balance", Value::is_string),
        ],
    )?;
    require_fields(
        &balance,
        &[
            ("spendable", Value::is_string),
            ("locked", Value::is_string),
            ("pending", Value::is_string),
            ("total", Value::is_string),
        ],
    )?;
    Ok(WalletSnapshot { info, balance })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LookupKind {
    Block,
    Transaction,
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}

fn lookup_request(kind: LookupKind, query: &str) -> Result<(&'static str, Value), String> {
    let query = query.trim();
    match kind {
        LookupKind::Block => {
            if let Ok(height) = query.parse::<u32>()
                && query.bytes().all(|c| c.is_ascii_digit())
            {
                return Ok(("getblock", json!([height, 2])));
            }
            if is_hash(query) {
                Ok(("getblock", json!([query.to_ascii_lowercase(), 2])))
            } else {
                Err("Enter a block height (0–4294967295) or a 64-character block hash".into())
            }
        }
        LookupKind::Transaction if is_hash(query) => Ok((
            "getrawtransaction",
            json!([query.to_ascii_lowercase(), true]),
        )),
        LookupKind::Transaction => Err("Enter a 64-character transaction hash".into()),
    }
}

pub(crate) async fn lookup(
    credentials: Credentials,
    kind: LookupKind,
    query: String,
) -> Result<Value, String> {
    let (method, params) = lookup_request(kind, &query)?;
    let result = Client::new(&credentials)?.call(method, params).await?;
    if !result.is_object() {
        return Err("The daemon returned an unexpected explorer result".into());
    }
    Ok(result)
}

/// Converts raw Kryptos units to K without rounding through floating point.
pub(crate) fn format_bus(raw: &str) -> String {
    let Ok(raw) = raw.parse::<u64>() else {
        return "—".to_owned();
    };
    format!("{}.{:08}", raw / 100_000_000, raw % 100_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[test]
    fn accepts_only_explicit_local_http_rpc_ports() {
        for endpoint in ["http://127.0.0.1:18443/", "http://127.0.0.1:18447"] {
            assert!(endpoint_url(endpoint).is_ok());
        }
        for endpoint in [
            "http://localhost:18443/",
            "http://127.0.0.2:18443/",
            "http://127.0.0.1:0/",
            "http://127.0.0.1:65536/",
            "http://127.0.0.1/",
            "https://127.0.0.1:18443/",
            "http://127.0.0.1:18443@evil.example/",
            "http://127.0.0.1:18443/?token=secret",
            "http://127.0.0.1:18443/#secret",
            "http://127.0.0.1:18443/explorer",
            "http://127.1:18443/",
            "http://2130706433:18443/",
        ] {
            assert!(endpoint_url(endpoint).is_err(), "accepted {endpoint}");
        }
    }

    #[test]
    fn credentials_and_errors_do_not_reveal_tokens() {
        let credentials = Credentials {
            endpoint: "http://127.0.0.1:18443/".into(),
            token: "secret-rpc-token".into(),
        };
        assert!(!format!("{credentials:?}").contains(&credentials.token));
        let error = decode_response(
            json!({"jsonrpc":"2.0", "id":1,
                "error":{"code":-32603, "message":credentials.token}}),
            1,
        )
        .unwrap_err();
        assert!(!error.contains("secret-rpc-token"));
        assert!(
            Client::new(&Credentials {
                token: "secret\r\nInjected: value".into(),
                ..credentials
            })
            .is_err()
        );
    }

    #[test]
    fn rejects_mismatched_or_ambiguous_rpc_envelopes() {
        assert!(decode_response(json!({"jsonrpc":"2.0","id":2,"result":42}), 1).is_err());
        assert!(decode_response(json!({"id":1,"result":42}), 1).is_err());
        assert!(decode_response(json!({"jsonrpc":"2.0","id":1}), 1).is_err());
        assert!(
            decode_response(
                json!({"jsonrpc":"2.0","id":1,"result":42,"error":{"code":-1}}),
                1,
            )
            .is_err()
        );
        assert_eq!(
            decode_response(json!({"jsonrpc":"2.0","id":1,"result":42}), 1).unwrap(),
            json!(42)
        );
    }

    #[test]
    fn lookup_matches_rhombus_rpc_parameter_types() {
        assert_eq!(
            lookup_request(LookupKind::Block, "0").unwrap(),
            ("getblock", json!([0, 2]))
        );
        assert!(lookup_request(LookupKind::Block, "4294967296").is_err());
        assert!(lookup_request(LookupKind::Block, "-1").is_err());
        assert!(lookup_request(LookupKind::Block, "+1").is_err());
        assert!(lookup_request(LookupKind::Transaction, "42").is_err());
        let hash = "A".repeat(64);
        assert_eq!(
            lookup_request(LookupKind::Transaction, &hash).unwrap(),
            ("getrawtransaction", json!(["a".repeat(64), true]))
        );
    }

    #[test]
    fn bus_formatting_retains_atomic_precision() {
        assert_eq!(format_bus("0"), "0.00000000");
        assert_eq!(format_bus("1"), "0.00000001");
        assert_eq!(format_bus("123456789"), "1.23456789");
        assert_eq!(format_bus("18446744073709551615"), "184467440737.09551615");
        assert_eq!(format_bus("1.5"), "—");
    }

    async fn mock(response: String) -> (Credentials, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0; 1024];
            loop {
                let size = socket.read(&mut chunk).await.unwrap();
                assert!(size > 0);
                buffer.extend_from_slice(&chunk[..size]);
                if let Some(end) = buffer.windows(4).position(|s| s == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&buffer[..end]);
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap()
                        .parse()
                        .unwrap();
                    if buffer.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(buffer).unwrap()
        });
        (
            Credentials {
                endpoint,
                token: "test-only-token".into(),
            },
            server,
        )
    }

    #[tokio::test]
    async fn sends_authenticated_json_rpc_and_reads_response() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"height":42}}"#;
        let (credentials, server) = mock(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ))
        .await;
        let result = Client::new(&credentials)
            .unwrap()
            .call("getblockchaininfo", json!([]))
            .await
            .unwrap();
        assert_eq!(result["height"], 42);
        let request = server.await.unwrap();
        assert!(request.starts_with("POST / HTTP/1.1\r\n"));
        assert!(request.contains("authorization: Bearer test-only-token\r\n"));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({"jsonrpc":"2.0", "id":1, "method":"getblockchaininfo", "params":[]})
        );
    }

    #[tokio::test]
    async fn refuses_redirects_and_oversized_responses() {
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\n\r\n"
                .to_owned(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_RESPONSE_BYTES + 1
            ),
        ] {
            let (credentials, server) = mock(response).await;
            assert!(
                Client::new(&credentials)
                    .unwrap()
                    .call("getmetrics", json!([]))
                    .await
                    .is_err()
            );
            server.await.unwrap();
        }
    }

    // These fixtures mirror upstream runtime/src/jsonrpc.rs and walletd.rs at
    // Rhombus 7a97c456. Serve all snapshot calls, including concurrent requests,
    // and echo their individual JSON-RPC IDs just as the real daemon does.
    async fn schema_server(
        fixtures: Vec<(&'static str, Value)>,
    ) -> (Credentials, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut fixtures: std::collections::BTreeMap<_, _> = fixtures.into_iter().collect();
            while !fixtures.is_empty() {
                let (mut socket, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0; 1024];
                let request = loop {
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0);
                    buffer.extend_from_slice(&chunk[..size]);
                    if let Some(end) = buffer.windows(4).position(|s| s == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&buffer[..end]);
                        let length: usize = header
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(str::to_owned)
                            })
                            .unwrap()
                            .parse()
                            .unwrap();
                        if buffer.len() >= end + 4 + length {
                            assert!(header.contains("authorization: Bearer fixture-token"));
                            break serde_json::from_slice::<Value>(&buffer[end + 4..]).unwrap();
                        }
                    }
                };
                assert_eq!(request["params"], json!([]));
                let result = fixtures
                    .remove(request["method"].as_str().unwrap())
                    .expect("unexpected or duplicate RPC method");
                let body =
                    json!({"jsonrpc":"2.0", "id":request["id"], "result":result}).to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (
            Credentials {
                endpoint,
                token: "fixture-token".into(),
            },
            server,
        )
    }

    #[tokio::test]
    async fn node_snapshot_matches_rhombus_method_schemas() {
        let hash = "a".repeat(64);
        let fixtures = vec![
            (
                "getblockchaininfo",
                json!({"height":42,"network":"test","best_block_hash":hash,"syncing":false}),
            ),
            (
                "getmetrics",
                json!({"height":42,"peers_inbound":1,"peers_outbound":2,"uptime_seconds":123,"mining_ready":true}),
            ),
            ("getnetworkinfo", json!({"network":"test","connections":3})),
            (
                "getmininginfo",
                json!({"height":42,"subsidy":"5000000000","block_interval":300}),
            ),
            (
                "getpeerinfo",
                json!([{"direction":"outbound","ready":true,"addr":null,"target":"peer.onion:9000"}]),
            ),
            ("getrawmempool", json!([hash])),
            ("getmempoolinfo", json!({"size":1,"capacity":1000})),
        ];
        let (credentials, server) = schema_server(fixtures).await;
        let snapshot = fetch_node(credentials).await.unwrap();
        server.await.unwrap();
        assert_eq!(snapshot.chain["height"], 42);
        assert_eq!(snapshot.peers.len(), 1);
        assert_eq!(snapshot.mempool, vec![hash]);
        assert_eq!(
            format_bus(snapshot.mining["subsidy"].as_str().unwrap()),
            "50.00000000"
        );
    }

    #[tokio::test]
    async fn wallet_snapshot_retains_raw_amounts_and_nullable_receive_address() {
        let fixtures = vec![
            (
                "getwalletinfo",
                json!({"network":"main","synced":false,"balance":"123456789","receive_address":null,"address_rotation_exhausted":true,"scanned_height":null,"node_height":0}),
            ),
            (
                "getbalance",
                json!({"spendable":"100000000","locked":"20000000","pending":"3456789","total":"123456789"}),
            ),
        ];
        let (credentials, server) = schema_server(fixtures).await;
        let snapshot = fetch_wallet(credentials).await.unwrap();
        server.await.unwrap();
        assert!(snapshot.info["receive_address"].is_null());
        assert_eq!(snapshot.balance["total"], "123456789");
        assert_eq!(
            format_bus(snapshot.balance["pending"].as_str().unwrap()),
            "0.03456789"
        );
    }

    #[tokio::test]
    async fn wallet_rejects_non_rhombus_balance_schema() {
        let fixtures = vec![
            (
                "getwalletinfo",
                json!({"network":"main","synced":true,"balance":"100000000"}),
            ),
            ("getbalance", json!(1.0)),
        ];
        let (credentials, server) = schema_server(fixtures).await;
        let error = fetch_wallet(credentials).await.unwrap_err();
        server.await.unwrap();
        assert_eq!(error, "The daemon returned an unexpected data format");
    }
}
