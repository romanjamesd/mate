# mate

A P2P echo server implementation in Rust for testing cryptographic message exchange.

## Features

- Ed25519 cryptographic identity generation and management
- P2P TCP networking with message framing
- Signed message envelopes for integrity verification
- CLI interface for server and client operations

## Usage

```bash
# Initialize a new identity
cargo run -- init

# Show current identity info
cargo run -- info

# Start echo server
cargo run -- serve --bind 127.0.0.1:8080

# Connect to a peer
cargo run -- connect 127.0.0.1:8080
```

## Development

This project implements the foundation for P2P chess gameplay, starting with a simple echo server to validate the networking and cryptographic components.