# mate

A fully decentralized peer-to-peer chess game with cryptographic security and offline-first gameplay.

## Features

### Core Gameplay
- **True P2P Chess**: Play directly with peers without any central servers
- **Offline-first Design**: All game data stored locally, sync when both players are online
- **Cryptographic Integrity**: Every move is cryptographically signed with Ed25519, preventing tampering
- **Resilient Networking**: Games continue seamlessly despite network interruptions
- **Cross-platform**: Works on macOS, Linux, and Windows

### Game Management
- Send game invitations directly to peer addresses
- Accept/decline invitations with color preferences
- Multiple concurrent games with different opponents
- Automatic game synchronization when peers reconnect
- Complete move history with cryptographic proof

### Security
- Ed25519 digital signatures ensure move authenticity
- Each player has a unique cryptographic identity
- Tamper-proof game history
- No trusted third parties required

## Installation

```bash
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
- **Conflict Resolution**: Cryptographic signatures prevent cheating

### Network Protocol
- TCP connections with message framing
- Ed25519 signatures on all moves
- Automatic peer discovery on local networks
- Manual peer address exchange for internet play

### Security Model
- Each player generates a unique Ed25519 keypair
- All moves are signed with the player's private key
- Game history is tamper-proof and independently verifiable
- No trusted third parties or central authorities

## Configuration

Mate stores configuration and game data in:
- **Linux**: `~/.config/mate/`
- **macOS**: `~/Library/Application Support/mate/`
- **Windows**: `%APPDATA%\mate\`

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.