# mate

A fully decentralized peer-to-peer chess game with cryptographic security and offline-first gameplay.

## Features

### Core Gameplay
- **True P2P Chess**: Play directly with peers without any central servers
- **Offline-first Design**: All game data stored locally, sync when both players are online
- **Message Authentication**: Move envelopes are signed with Ed25519 and verified on receipt
- **Resilient Networking**: Games continue seamlessly despite network interruptions
- **Cross-platform**: Works on macOS, Linux, and Windows

### Game Management
- Send game invitations directly to peer addresses
- Accept/decline invitations with color preferences
- Multiple concurrent games with different opponents
- Automatic game synchronization when peers reconnect
- Local move history in ply order

### Security
- Ed25519 digital signatures ensure move authenticity
- Each player has a unique cryptographic identity
- Received envelope signers are bound to the authenticated connection peer
- No trusted third parties required

## Installation

```bash``
# Install from crates.io
cargo install mate

# Or build from source
git clone https://github.com/username/mate
cd mate
cargo build --release
```

## Quick Start

```bash
# 1. Initialize your identity
mate key generate

# 2. Check your peer ID
mate key info

# 3. Start listening for connections
mate serve --bind 0.0.0.0:8080

# 4. Share your address (IP:8080) and peer ID with a friend
```

## Usage

### Identity & Key Management
```bash
# Generate a new cryptographic identity
mate key generate

# Show your peer ID and key information
mate key info

# Show where keys are stored
mate key path
```

### Network & Connection
```bash
# Start server to accept connections
mate serve --bind 0.0.0.0:8080

# Connect to another peer for testing
mate connect 192.168.1.100:8080

# Send a specific message when connecting
mate connect 192.168.1.100:8080 --message "Hello, peer!"
```

### Game Management (Future)
```bash
# Invite someone to play (they need to be running `mate serve`)
mate invite 192.168.1.100:8080

# View pending invitations and active games
mate games

# Accept a game invitation
mate accept game_abc123

# Show all known peers
mate peers
```

If an invitation's acknowledgement is lost, the game remains pending. Inspect
`mate games` and resend the persisted invitation with `mate retry-invite <game-id>`.
Retries reuse the original payload and require the original opponent's authenticated
peer identity, including after restarting the CLI.

### Playing Chess
```bash
# Make a move using algebraic notation
mate move e4 game_abc123
mate move Nf3 game_abc123
mate move O-O game_abc123

# View current board position
mate board game_abc123

# View complete game history
mate history game_abc123

# Force synchronization of all games
mate sync
```

### Example Game Session
```bash
$ mate games
Active Games:
  game_abc123 vs alice_def456 [Your turn] - White
  game_xyz789 vs bob_ghi012   [Waiting]   - Black

$ mate board game_abc123
  ┌─────────────────────┐
8 │ r n b q k b n r │
7 │ p p p p . p p p │
6 │ . . . . . . . . │
5 │ . . . . p . . . │
4 │ . . . . P . . . │
3 │ . . . . . . . . │
2 │ P P P P . P P P │
1 │ R N B Q K B N R │
  └─────────────────────┘
    a b c d e f g h

$ mate move Nf3 game_abc123
Move played: Nf3
Waiting for opponent...

$ mate history game_abc123
Game: game_abc123
1. e4 e5
2. Nf3 ...
```

## How It Works

### Decentralized Architecture
- **No Central Server**: Players connect directly to each other
- **Local Database**: SQLite stores all game state locally
- **P2P Synchronization**: Games sync when both players are online
- **Message Verification**: Cryptographic signatures authenticate received envelopes

### Network Protocol
- TCP connections with message framing
- Ed25519 signatures on all moves
- Automatic peer discovery on local networks
- Manual peer address exchange for internet play

Signed application envelopes must match the connection's authenticated peer.
This includes invite, accept, and decline responses as well as Move responses.
A signer mismatch is terminal on every CLI path: the connection is dropped and
the request is not automatically retried or queued; interactive sessions end.
An untrusted response does not prove the request was undelivered. Check the peer
address and game state before deciding whether to retry manually.

`Move` carries `game_id`, `ply`, `chess_move`, and `board_state_hash` in the
signed payload. `ply` is a required one-based half-move number: White's first
move is 1 and Black's first is 2. The hash describes the board after that move.
Chess history is read in ply order and replay rejects gaps, duplicates,
row/payload mismatches, application failures, and incorrect post-move hashes.
Generic message history uses local insertion `id` order; timestamps are only
display/audit metadata. Move application does not yet enforce full chess
legality or king safety.

`MoveAck` carries `game_id`, `ply`, and `move_digest`, a lowercase hex SHA-256
digest identifying the exact Move payload. The receiver acknowledges only after
an atomic commit, or for an exact retry from the same authenticated sender.
Rejections return `GameDecline`. The sender verifies the opponent identity,
response signer, and ACK fields before committing locally and reporting success.
Legacy Move and MoveAck wire formats are incompatible; both peers must use the
updated protocol.

Moves are not queued for later delivery. If delivery is uncertain, or the
opponent accepted but local persistence failed, follow the CLI's retry guidance.
With unchanged local history (and any local storage problem resolved), repeating
the same move reproduces the payload and an already committed move is acknowledged
again. If local history has advanced, compare histories before moving again;
recovery is not automatic.

### Security Model
- Each player generates a unique Ed25519 keypair
- All moves are signed with the player's private key
- Signatures are verified on receipt; stored Move records currently use
  `local`/`remote` signature placeholders and cannot independently prove the
  original envelope signer
- No trusted third parties or central authorities

Signer binding does not add broader handshake replay protection or automatic
recovery. Timestamp freshness checks do not deduplicate messages within the
accepted window.

## Configuration

Mate stores configuration and game data in:
- **Linux**: `~/.config/mate/`
- **macOS**: `~/Library/Application Support/mate/`
- **Windows**: `%APPDATA%\mate\`

### Upgrading a development database

Schema version 3 requires an explicit `ply` for every chess Move. A database
with older Move rows cannot be upgraded; opening it reports the row count and
actual file path. Stop any running `mate serve` process, keep a copy if you need
the old history, then delete the database file at the printed path. The next
database-using command creates a fresh schema-version-3 database. This discards
the database's games and message history; keep your identity keys.
Migration failure leaves all existing data unchanged. Databases containing
only invites, accepts, or other non-Move messages upgrade in place.
`MATE_DATA_DIR` overrides the default database directory.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.
