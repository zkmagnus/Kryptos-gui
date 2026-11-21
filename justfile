alias b := build
alias c := check
alias f := fmt
alias r := run
alias rr := run-release
alias t := test

_default:
    @just --list

# Build Kryptos
build:
    cargo build --locked

# Check formatting, compilation, linting, and tests
check:
    cargo fmt --all -- --check
    cargo check --locked
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked

# Format the client with stable rustfmt
fmt:
    cargo fmt --all

# Run the desktop client
run:
    cargo run --locked

# Run the desktop client in release mode
run-release:
    cargo run --release --locked

# Run unit and local RPC fixture tests
test:
    cargo test --locked
