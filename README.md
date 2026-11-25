# Kryptos

[Kryptos](https://github.com/zkmagnus/Kryptos-gui) is a desktop status dashboard for [Kryptos](https://github.com/zkmagnus/Kryptos), the shielded `$K` blockchain. It connects to the node's authenticated, line-oriented control service over IPv4 loopback. The GUI does not use HTTP or JSON-RPC.

Kryptos is experimental. Its transfers use Halo2 proofs and ML-KEM coin-payload encryption; ML-KEM does not make the proof system or spending authorization fully post-quantum.

## What the GUI supports

| Tab | Data and actions |
| --- | --- |
| Overview | Chain height, sync state, tip hash, peer count, mempool count, and blocks mined by this node |
| Network | Authenticated peer count and sync status |
| Mining | Local block counts and current chain status; detailed readiness is not exposed by control |
| Explorer | Block hash by height and transaction pool/confirmation status |
| Wallet | CLI guidance for balance, address, send, and recovery commands |
| Settings | Connect to the local node control socket |
| About | Project and protocol information |

The current control protocol intentionally exposes a small read-only data set to
this dashboard. It does not provide peer details, a mempool transaction list,
block bodies, or wallet balances. Wallet commands open the wallet file directly
through the Kryptos CLI; do not open the same wallet file in two processes.

## Build and run

Install a current stable [Rust toolchain](https://rustup.rs). In a standalone
GUI checkout, build the desktop client from this directory:

```sh
cargo run --release --locked
```

This builds the standalone desktop client named `kryptos`. It does not compile
the node or proof system. A graphical desktop session is required to open it.

Keep the GUI as a sibling of the node and dependency repositories:

```text
workspace/
├── Kryptos/
├── dependency/
└── gui/
```

Run GUI Cargo and Git commands from `gui/`. It has its own manifest, lockfile,
and Git history; the node's offline Cargo configuration and dependency snapshot
do not apply to this standalone checkout. Commit GUI changes here separately
from node or dependency changes.

## Connect a Kryptos node

Build the node separately using its
[build instructions](https://github.com/zkmagnus/Kryptos/blob/master/BUILD.md).
Run the node command from the upstream node repository:

```sh
cargo build --release --locked -p runtime --bins
mkdir -p local-test
chmod 700 local-test

target/release/kryptos run --network test --datadir local-test/node-a \
  --p2p-port 19000 --rpc 127.0.0.1:18440
```

Kryptos has no built-in public bootstrap peers. To join an existing chain,
configure valid v3 onion peers with `--addnode`, `--connect`, or `--seed`,
following the [operations guide](https://github.com/zkmagnus/Kryptos/blob/master/docs/operations.md).

In the GUI **Settings** tab, use the local control address `127.0.0.1:18440`
and enter the contents of `local-test/node-a/rpc.token`. The default node
control port is 18440. The token is required for each authenticated control
session and is kept in memory only.

## Wallet commands

Create or restore a wallet with the Kryptos CLI. Keep the printed recovery
phrase private. For example:

```sh
target/release/kryptos create --network test --wallet local-test/wallet.redb
target/release/kryptos balance --network test --wallet local-test/wallet.redb \
  --rpc 127.0.0.1:18440 --rpc-token-file local-test/node-a/rpc.token
target/release/kryptos address --network test --wallet local-test/wallet.redb
```

Address rotation, scanning, sending, and recovery remain under the CLI and
wallet library; see the upstream
[address rotation guide](https://github.com/zkmagnus/Kryptos/blob/master/docs/address-rotation.md).

## Settings and credentials

The GUI saves the control address in
`<platform configuration directory>/rhombus/settings.toml`, using
`dirs::config_dir()`. On Linux this is normally
`~/.config/rhombus/settings.toml`. Existing URL-based settings from the former
HTTP client fall back to the new default address; enter and save the control
address again if needed.

The node token is masked in the GUI, kept in memory, and excluded from the
settings file. Enter it again after reopening the application. Connecting
requires an explicit user action; a saved address does not trigger automatic
connections. Only IPv4 loopback addresses are accepted.

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

## Credits and license

This desktop client is adapted from Bonsai, [Luis Schwab's desktop Bitcoin wallet and embedded node showcase](https://github.com/luisschwab/bonsai). This GUI adaptation retains the Bonsai project's [MIT license](LICENSE) and attribution.

The upstream Kryptos protocol and node use GPL-3.0-or-later. See the
[Kryptos repository](https://github.com/zkmagnus/Kryptos) for protocol source,
licenses, and operational documentation.
