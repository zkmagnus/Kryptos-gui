# Kryptos GUI validation

The GUI now uses the node's authenticated line-oriented control service on an
IPv4 loopback socket. It requests `status`, `hash <height>`, and
`txstatus <txid>`. It has no HTTP or JSON-RPC client and no wallet-daemon
connection. Wallet activity remains available through the Kryptos CLI.

The control client validates loopback addresses and token shape, authenticates
before sending commands, bounds reply size, and uses a request timeout. Its
status view only displays fields returned by the node control protocol; detailed
peer data, block bodies, mempool IDs, and wallet balances are not available from
that protocol.

The prior validation report covered HTTP fixtures and the previous client. The
line-protocol migration compiled with `cargo +1.100.0 check --offline`; the GUI
test suite was not run for this change.
