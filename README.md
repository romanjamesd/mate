# mate

`mate` is a Rust CLI for signed peer-to-peer messaging and an in-progress peer-to-peer chess workflow.

The repository already has a solid foundation for identities, message signing, transport, and SQLite-backed local state. The chess command set exists today, but parts of the gameplay/runtime path are still under active development, so this README distinguishes implemented behavior from experimental behavior.

## Status

- Stable foundation: Ed25519 identities, signed message types, TCP client/server transport, local SQLite persistence, and broad automated test coverage
- Working network tools: `serve` and `connect` for peer connectivity and echo-style round-trip testing
- Experimental chess CLI: `games`, `board`, `invite`, `accept`, `move`, and `history`
- Current limitation: the chess command surface is ahead of the fully wired end-to-end runtime, so some multiplayer chess flows are not yet production-ready

## Current Features

### Identity and Security
- Local Ed25519 identity generation and storage
- Signed wire/message layer for peer communication
- Per-peer identity information and reproducible peer IDs
- Secure key-file permissions on Unix systems

### Networking
- TCP server for accepting inbound peer connections
- Interactive client session for manual peer messaging
- One-shot connect-and-send mode for latency and echo testing
- Connection lifecycle handling and reconnect-oriented client behavior

### Local Chess State
- SQLite-backed game and message storage
- CLI commands for listing games, viewing history, and issuing invites/moves
- Board and move-history views driven from stored game/message records
- Configurable data/config directories via environment variables

## What Is Experimental

The chess workflow is present in the CLI, but not every layer is fully connected yet. In particular:

- `board` and `history` are useful for inspecting stored state, but board reconstruction is still simplified
- `move` currently performs basic validation rather than full legal-move enforcement
- live invite/accept/move handling across the network is still being hardened

If you want the most reliable current functionality, use `serve` and `connect` first.

## Requirements

- Rust stable
- `cargo`, `rustfmt`, and `clippy`

The repo pins the stable toolchain in `rust-toolchain.toml` and expects `rustfmt` and `clippy` to be available.

## Build From Source

```bash
git clone <your-fork-or-remote>
cd mate
cargo build
```

For an optimized binary:

```bash
cargo build --release
```

## Quick Start

### 1. Generate an identity

```bash
mate key generate
mate key info
mate key path
```

### 2. Start a peer server

```bash
mate serve --bind 127.0.0.1:8080
```

### 3. Connect from another terminal or machine

Interactive session:

```bash
mate connect 127.0.0.1:8080
```

One-shot echo test:

```bash
mate connect 127.0.0.1:8080 --message "ping"
```

The interactive `connect` session supports:

- `help` to show session commands
- `info` to show session statistics
- `quit` or `exit` to close the session

## CLI Commands

### Identity

```bash
mate key generate
mate key info
mate key path
```

Deprecated compatibility commands still exist:

```bash
mate init
mate info
```

### Networking

```bash
mate serve --bind 127.0.0.1:8080
mate connect <host:port>
mate connect <host:port> --message "hello"
```

### Chess Commands

These commands are implemented in the CLI and use the local app/database state.

```bash
mate games
mate board
mate board --game-id <game-id>
mate invite <host:port>
mate invite <host:port> --color white
mate accept <game-id>
mate accept <game-id> --color black
mate move e4
mate move Nf3 --game-id <game-id>
mate history
mate history --game-id <game-id>
```

Supported color values for invite/accept are:

- `white`
- `black`
- `random`

### Command Notes

- `games` lists locally stored games and their status
- `board` defaults to the most relevant local game if `--game-id` is omitted
- `move` defaults to an active local game if `--game-id` is omitted
- `history` defaults to the most relevant local game if `--game-id` is omitted
- `invite`, `accept`, and `move` depend on both local state and the network layer, so expect rough edges while the chess runtime is still evolving

## Data and Configuration

By default, `mate` uses platform-specific directories via the Rust `directories` crate.

The application stores:

- identity key: `identity.key`
- database: `database.sqlite`
- config: `config.toml`

Environment overrides:

- `MATE_DATA_DIR` overrides the data directory used for the identity key and database
- `MATE_CONFIG_DIR` overrides the configuration directory

On macOS, the default base location comes from `ProjectDirs::from("dev", "mate", "mate")`, which typically resolves under `~/Library/Application Support/`.

## Development

Common commands from the repository root:

```bash
cargo build
cargo test
make check
make ci
make test-ci-safe
```

What they do:

- `cargo build`: build the crate and CLI binary
- `cargo test`: run the local test suite
- `make check`: run formatting and Clippy checks
- `make ci`: run the CI-style format, lint, and test flow
- `make test-ci-safe`: run tests single-threaded for race-sensitive debugging

## Project Layout

- `src/chess/`: chess board, move, and position logic
- `src/cli/`: CLI parsing, app lifecycle, display, and command handlers
- `src/crypto/`: identity and secure key storage
- `src/messages/`: wire and chess message types
- `src/network/`: client, connection, and server transport
- `src/storage/`: SQLite database layer and models
- `tests/`: unit, integration, security, performance, and storage test coverage
- `scripts/`: local CI/debug helper scripts

## Testing Focus

The test suite is broad and includes:

- unit tests for chess, CLI, crypto, messages, and networking components
- integration tests for CLI, networking, storage, and chess protocol behavior
- security tests for error handling and denial-of-service protection
- performance tests for throughput and protocol stress scenarios

## Notes For Contributors

This repository is actively evolving. When updating docs or examples, prefer describing the current code path over the intended end state, especially for the chess multiplayer workflow.