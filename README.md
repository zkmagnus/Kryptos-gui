# Kryptos

[Kryptos](https://github.com/zkmagnus/Kryptos-gui) is a desktop dashboard for [Kryptos](https://github.com/zkmagnus/Kryptos), the
shielded `$K` blockchain. Kryptos uses Rust and [Iced](https://iced.rs) to show
node activity, Tor peers, mining readiness, blocks, transactions, and wallet
balances through Kryptos's authenticated local JSON-RPC interfaces.

This desktop client connects to the Kryptos node and wallet through their local JSON-RPC interfaces.
Kryptos is experimental. Its transfers use Halo2 proofs and ML-KEM coin payload
encryption; ML-KEM does not make the proof system or spending authorization fully
post-quantum.

See [validation notes](docs/validation.md) for the checks performed and their limits.

## What the GUI supports

| Tab | Data and actions |
| --- | --- |
| Overview | Chain height, synchronization, tip hash, peers, mempool, and uptime |
| Network | Tor peer endpoints, connection direction, handshake readiness, height, and latency |
| Mining | Readiness and wait reasons, local block outcomes, work rate, and mining parameters |
| Explorer | Look up a block by height or hash, or a transaction by its hash |
| Wallet | Spendable, locked, pending, and total balance; scan status; current receive address and copy action |
| Settings | Configure and connect local node and wallet RPC endpoints |
| About | Project and protocol information |

Node and wallet queries are read-only. The GUI does not start or stop node
processes, change peers, enable mining, create wallets, or send transactions.
These operations remain with the Kryptos binaries. The former embedded
Bitcoin/Floresta backend and Utreexo views are replaced by Kryptos RPC views.

Public block data cannot reveal shielded recipients or balances. Wallet data
comes from the separate wallet daemon that holds the corresponding wallet keys.
K amounts use eight decimal places, with integer conversion from the RPC's
smallest-unit strings.

## Build and run

Install a current stable [Rust toolchain](https://rustup.rs). Build Kryptos from this repository:

```sh
cargo run --release --locked
```

This repository builds a standalone desktop client named `kryptos` at
`target/release/kryptos`. Building it does not require a sibling node checkout,
and it does not compile the Kryptos node or its proof system. A graphical
desktop session is required to open the application.

## Connect a Kryptos node

Build the Kryptos node separately using its
[build instructions](https://github.com/zkmagnus/Kryptos/blob/master/BUILD.md).
Run the following example **from the upstream node repository**, not this
desktop client repository:

```sh
cargo build --release --locked -p runtime --bins
mkdir -p local-test
chmod 700 local-test

target/release/kryptos run --network test --datadir local-test/node-a \
  --p2p-port 19000 --rpc 127.0.0.1:18444 --rpc-http 127.0.0.1:18443
```

This starts a local test-network node without enabling mining. Kryptos has no
built-in public bootstrap peers. To join an existing chain, configure its valid
v3 onion peers with `--addnode`, `--connect`, or `--seed`, following the
[operations guide](https://github.com/zkmagnus/Kryptos/blob/master/docs/operations.md).
Tor service readiness alone does not establish chain synchronization.

In the desktop client's **Settings** tab:

1. Set the node endpoint to `http://127.0.0.1:18443/`.
2. Enter the contents of the node's `local-test/node-a/rpc.token` file in the
   node token field.
3. Connect to load the node dashboard.

Use the HTTP JSON-RPC port (`18443` in this example), not the line-protocol RPC
port (`18444`). Node and wallet endpoints must use `http://127.0.0.1:<port>/`.
The client disables HTTP proxies and redirects. Kryptos itself handles peer and
remote-node traffic through embedded Arti; the desktop client connects only to local RPC.

## Connect a wallet

The wallet view is optional and uses its own connection and token. Create or
restore a wallet with the Kryptos CLI first. For example, from the upstream node repository:

```sh
target/release/kryptos create --network test --wallet local-test/wallet.redb
```

Keep the printed recovery phrase private. After the CLI releases the database,
start the wallet daemon. Replace the onion placeholder with the node's hostname
reported after service readiness:

```sh
KRYPTOS_ONION_A='<node-a-v3-hostname>.onion'
target/release/wallet --network test --wallet local-test/wallet.redb \
  --node "$KRYPTOS_ONION_A:18444" --datadir local-test/wallet-daemon \
  --node-token-file local-test/node-a/rpc.token --listen 127.0.0.1:18447
```

The wallet daemon uses independent Tor state to reach the node. Its local HTTP
endpoint defaults in the desktop client to `http://127.0.0.1:18447/`. Enter the daemon's
`wallet-rpc.token`, stored beside the wallet database, in the desktop client's **wallet token**
field, then connect. This token differs from the node token. Keep the wallet and
node on the same network, and do not open the wallet database concurrently from
another process.

Kryptos displays the wallet's current receive address without allocating a new
one. Address rotation, scanning, and recovery rules remain under the Kryptos
wallet's control; see its
[address rotation guide](https://github.com/zkmagnus/Kryptos/blob/master/docs/address-rotation.md).

## Settings and credentials

**Save settings** persists endpoint addresses in
`<platform configuration directory>/rhombus/settings.toml`, using
`dirs::config_dir()`. On Linux this is normally
`~/.config/rhombus/settings.toml`.

RPC tokens are masked in the GUI, kept in memory, and excluded from the settings
file. Enter them again after reopening Kryptos. Connecting requires an explicit
user action; saved endpoints do not trigger automatic connections. Credentials
are separate for the node and the wallet.

## Development

[`just`](https://github.com/casey/just) is optional:

```sh
just                 # List commands
just run             # Run a debug build
just run-release     # Run a release build
just check           # Formatting, compilation, linting, and tests
```

The equivalent checks are:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

RPC tests use local fixtures and mock responses. They do not validate live Tor
connectivity, consensus, wallet recovery, or proof generation in Kryptos.

## Credits and license

This desktop client is adapted from Bonsai, [Luis Schwab's desktop Bitcoin wallet and embedded node
showcase](https://github.com/luisschwab/bonsai). This GUI adaptation retains the
Bonsai project's [MIT license](LICENSE) and attribution.

The upstream Kryptos protocol and daemons use GPL-3.0-or-later. This client
communicates with its external binaries over JSON-RPC. See the
[Kryptos repository](https://github.com/zkmagnus/Kryptos) for protocol source,
licenses, and operational documentation.
