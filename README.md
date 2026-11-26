# Kryptos

[Kryptos](https://github.com/zkmagnus/Kryptos-gui) is a desktop application with a built-in node for [Kryptos](https://github.com/zkmagnus/Kryptos), the shielded `$K` blockchain. The default mode runs the node and embedded Tor inside the desktop application through Rust APIs. An optional external-node mode uses the authenticated, line-oriented control service over IPv4 loopback. Neither mode uses HTTP or JSON-RPC.

Kryptos is experimental. Its transfers use Halo2 proofs and ML-KEM coin-payload encryption; ML-KEM does not make the proof system or spending authorization fully post-quantum.

## What the GUI supports

| Tab | Data and actions |
| --- | --- |
| Overview | Chain height, sync state, tip hash, peer count, mempool count, and blocks mined by this node |
| Network | Authenticated peer count and sync status |
| Mining | Local block counts and current chain status; built-in mining controls are pending |
| Explorer | Block hash by height and transaction pool/confirmation status |
| Wallet | CLI guidance for balance, address, send, and recovery commands |
| Settings | Start/stop the built-in node, select mainnet/testnet, configure onion peers, or connect an external local node |
| About | Project and protocol information |

The current dashboard exposes a small read-only data set from the node. It does not provide peer details, a mempool transaction list,
block bodies, or wallet balances. Wallet commands open the wallet file directly
through the Kryptos CLI; do not open the same wallet file in two processes.

## Build and run

The GUI pins Rust 1.100.0 and now compiles the Kryptos core and proof system.
Prepare the sibling core checkout at the revision in `CORE_REV`, its pinned
`../dependency/` snapshot, and the native prerequisites documented in
`../Kryptos/BUILD.md`. Build the desktop application from this directory:

```sh
cargo run --release --locked
```

This builds one desktop executable named `kryptos`, including the node and
embedded Tor. It does not launch a CLI subprocess or require a separate node
executable. A graphical desktop session is required to open it.

Keep the GUI as a sibling of the node and dependency repositories:

```text
workspace/
├── Kryptos/
├── dependency/
└── gui/
```

Run GUI Cargo and Git commands from `gui/`. It has its own manifest, lockfile,
and Git history. GUI registry dependencies are fetched from crates.io and cached
by Cargo; the core's offline registry configuration does not apply here.
After an online build or `cargo fetch --locked`, GUI checks can use `--offline`.
The RandomX source patch is loaded from `../dependency/patches/`, and the
Rust-compatible bindgen patch from `../dependency/registry/bindgen/`. The pinned
core dependencies must therefore be prepared before building. Commit GUI
changes here separately from node or dependency changes.

## Built-in node

Opening Kryptos starts its built-in node using saved network and peer settings.
The first startup prepares the cryptographic verifier before the node is ready;
the desktop remains responsive. No control token or RPC port is needed in this
mode, and mining is disabled in this stage.

In **Settings**, stop the node before changing mainnet/testnet or its onion
peers. Enter reachable v3 onion endpoints as `hostname.onion:19000`, separated
by commas, then save preferences and start the node. There are no built-in
public bootstrap peers yet. A node with no peers displays **Waiting for peers**;
it must not be treated as synchronized with a public network.

Blockchain and Tor state persist under
`<platform local data directory>/Kryptos/<main|test>/chain`:

- Linux normally uses `~/.local/share/Kryptos/<main|test>/chain`.
- Windows uses the local application-data directory.
- macOS uses the user's application-support directory.

The exact location is displayed in Settings. Existing CLI databases are not
moved or opened automatically. Keep `tor/state` when backing up the node to
preserve its service identity. A second application opening the same chain
fails visibly rather than sharing its database handle.

**Stop node** waits for orderly shutdown. Closing the window also waits for
shutdown before exiting. If stopping takes too long, the application retains
ownership and lets you retry; it does not force-kill the node. A startup still
in progress is canceled before it can become a running node after shutdown.

## External local node

Stop the built-in node, select **External node**, and enter a loopback control
address such as `127.0.0.1:18440` plus the node's `rpc.token`. Use **Connect node**
and save preferences if this should remain the mode on the next launch. The
token stays in memory and must be entered again after reopening. External mode
never starts a node automatically, and disconnecting or closing the application
leaves that independently managed node running.

## Wallet commands

Desktop wallet controls are not implemented in this stage. The examples below
use a separately built core CLI, from the `gui/` directory, with an external
node. The built-in node does not expose an RPC listener for CLI wallets. Keep
the printed recovery phrase private.

```sh
../Kryptos/target/release/kryptos create --network test --wallet local-test/wallet.redb
../Kryptos/target/release/kryptos balance --network test --wallet local-test/wallet.redb \
  --rpc 127.0.0.1:18440 --rpc-token-file local-test/node-a/rpc.token
../Kryptos/target/release/kryptos address --network test --wallet local-test/wallet.redb
```

Address rotation, scanning, sending, and recovery remain under the CLI and
wallet library; see the upstream
[address rotation guide](https://github.com/zkmagnus/Kryptos/blob/master/docs/address-rotation.md).

## Settings and credentials

The GUI saves the node mode, network, onion peers, and external control address in
`<platform configuration directory>/kryptos/settings.toml`, using
`dirs::config_dir()`. On Linux this is normally
`~/.config/kryptos/settings.toml`. Existing URL-based settings from the former
HTTP client fall back to the new default address; enter and save the control
address again if needed.

Connection preferences are validated for the selected node mode. An incomplete
external address does not block the built-in node, and unfinished onion peers
do not block external-node mode.

The node token is masked in the GUI, kept in memory, and excluded from the
settings file. Enter it again after reopening the application. External connections
require an explicit user action; a saved address does not trigger automatic
connections. The built-in node starts automatically when that mode is selected.
Only IPv4 loopback addresses are accepted for external control.

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

This desktop client is adapted from Bonsai, [Luis Schwab's desktop Bitcoin wallet and embedded node showcase](https://github.com/luisschwab/bonsai). The original GUI's [MIT license and attribution](LICENSE) are retained.

The application now links the Kryptos protocol and node, which use
GPL-3.0-or-later. The combined executable is distributed under
[GPL-3.0-or-later](LICENSE-GPL-3.0), with the original GUI attribution retained.
See the [Kryptos repository](https://github.com/zkmagnus/Kryptos) for core
source and operational documentation.
