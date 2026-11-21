# Kryptos validation

Compatibility reference: Rhombus `7a97c4560d01cb1dcdf251df7be1f940fca92f80`.

The desktop client uses the existing node and wallet JSON-RPC HTTP interfaces.
It builds independently of the Rhombus workspace. The previous Floresta/Bitcoin
source modules and dependencies have been removed from the active client.

## Automated checks

- `cargo fmt --all -- --check`
- `cargo build --locked`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked`: 15 tests passed.
- `git diff --check`

Tests cover exact BUS decimal conversion, localhost-only endpoint validation,
credential redaction and header injection rejection, authenticated HTTP calls,
JSON-RPC envelope validation, redirect and response-size limits, correct explorer
parameters, node and wallet schema fixtures, and stale asynchronous responses
following endpoint changes, disconnection, or query edits.

HTTP fixture tests bind ephemeral loopback ports. Environments that prohibit
local sockets must grant test processes permission to bind those ports.

## Desktop check

Opened the compiled Linux/X11 application with a temporary configuration and
two local fixture servers. Entered masked test credentials through the GUI,
connected both services, and verified that all seven node snapshot methods and
both wallet methods reached the fixtures. Inspected the Overview, Tor network,
Mining, Explorer, Wallet, and Settings views. Periodic refresh also reached the
fixtures. App processes and fixture servers were stopped after the check.

The desktop check used synthetic fixture data, including an intentionally fake
receiving address. It was performed before the application was renamed from
Bonsai to Rhombus. The renamed build passed compilation and lint checks; preview
images from the earlier branding are not published with this adaptation.

## Limits

The checks do not establish live Tor connectivity or compatibility with future
Rhombus RPC changes. No real wallet, balance, token, consensus process, mining
operation, or transaction broadcast was used. Start/stop, mining configuration,
wallet creation/recovery, and sending remain in the Rhombus binaries.
