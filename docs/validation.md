# Kryptos desktop node validation

Validated locally on Linux, November 26, 2026, using Rust 1.100.0 and the core
revision in `CORE_REV`.

## Stage one: embedded node lifecycle

The default desktop mode runs the node and embedded Tor through Rust APIs on a
separate Tokio runtime. It does not spawn a CLI process, bind an RPC listener,
or handle an RPC token. External local-node mode retains the authenticated
line-oriented control client.

Checks completed successfully:

```text
cargo check --offline --locked --all-targets
cargo fmt --all -- --check
cargo clippy --offline --locked --all-targets -- -D warnings
cargo test --offline --locked
cargo build --offline --locked
```

All 19 tests passed, covering:

- A real offline node opens a persistent chain, returns status and explorer
  lookups, stops, and reopens the same chain.
- A second owner cannot open an occupied database, but succeeds after shutdown.
- Dropping the service clients shuts down their owned node and releases storage.
- Stop invalidates an unpolled startup, preventing delayed startup after close.
- Malformed network and onion peer settings fail before opening storage.
- Stale asynchronous replies cannot reconnect a stopped node or replace an
  explorer query result.
- Network/mode edits are blocked while the desktop owns a node; Stop can cancel
  startup; a failed shutdown retains ownership for retry.
- Failed embedded refreshes hide stale chain data while allowing status retries.
- Credentials stay out of event debug output and persisted preferences; legacy
  endpoint-only settings load with embedded-node defaults.
- Incomplete settings for the inactive node mode do not block startup or saved
  preferences for the selected mode.
- The external client interoperates with the real core control service for
  authentication, status, block hashes, and transaction status. Invalid tokens
  never dispatch node requests, and malformed success prefixes are rejected.

The final full suite completed in approximately 17 seconds after compilation.
The two loopback control fixtures require permission to bind local sockets;
they passed outside the socket-restricted sandbox. GUI offline checks use the
previously populated Cargo registry cache, in addition to the local RandomX
and bindgen patches. They do not establish a fully vendored GUI build.

The debug executable was opened on an X11 desktop with isolated temporary
configuration/data directories and external-node mode selected. The Overview
rendered at 1200 x 850 and the minimum 1000 x 700 size, and a normal window close
exited successfully. This smoke test started no node and touched no user settings
or chain data. The release profile also preserves the core's integer overflow
checks; a release executable was not built in this verification pass.

An initial unoptimized test build exceeded the original 30-second startup
allowance while constructing the proof verifier. The final test profile matches
the core optimization settings, and startup has a separate bounded 180-second
allowance. Regular control requests remain bounded separately.

The initial registry build of `bindgen 0.58.1` emitted a future-incompatibility
notice about trailing semicolons in logging macros. The GUI now patches bindgen
to the already-corrected source in the pinned core dependency snapshot at
`../dependency/registry/bindgen/`. No dependency sources were edited for this fix.
The follow-up `cargo check --offline --all-targets --future-incompat-report`
passed and reported zero dependencies with future-incompatible warnings.
Ordinary legacy warnings from this old bindgen source are now visible because
it is a local path dependency; they do not fail this build check.

## Limits

Node lifecycle tests use isolated temporary databases with external networking
disabled. Public Tor bootstrap, peer synchronization, window closure while a
real embedded node is running, Windows/macOS builds, and installer packaging
have not been exercised in this session. The Iced event state and shutdown
service are tested separately.

No wallets, keys, transactions, or existing user chain databases were opened or
modified. Desktop wallet and mining controls remain outside this stage.
