use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mate")]
#[command(about = "A P2P chess client for playing chess over the network")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new identity (deprecated - use 'key generate' instead)
    Init,
    /// Show current peer ID and identity info (deprecated - use 'key info' instead)
    Info,
    /// Key management commands
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    /// Start the echo server
    Serve {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Connect to a peer
    Connect {
        /// Address to connect to
        address: String,
        #[arg(short, long)]
        message: Option<String>,
    },

    // New chess commands
    /// Show active games and their current status
    ///
    /// Lists all ongoing chess games with information about game state,
    /// opponents, and whose turn it is to move.
    ///
    /// Example: mate games
    Games,

    /// Show the chess board for a specific game
    ///
    /// Displays the current position of a chess game in ASCII format.
    /// If no game ID is provided, shows the most recently active game.
    ///
    /// Examples:
    ///   mate board
    ///   mate board --game-id abc123
    Board {
        /// Specific game ID to show. If not provided, shows most recent game
        #[arg(short, long)]
        game_id: Option<String>,
    },

    /// Invite someone to play a chess game
    ///
    /// Sends a chess game invitation to the specified peer address.
    /// You can optionally specify which color you want to play.
    ///
    /// Examples:
    ///   mate invite 127.0.0.1:8080
    ///   mate invite 127.0.0.1:8080 --color white
    ///   mate invite 127.0.0.1:8080 --color black
    Invite {
        /// Network address of the peer to invite (e.g., 127.0.0.1:8080)
        address: String,
        /// Color preference: 'white', 'black', or 'random' (default: random)
        #[arg(short, long)]
        color: Option<String>,
    },

    /// Accept a pending game invitation
    ///
    /// Accepts an incoming chess game invitation by game ID.
    /// You can optionally specify which color you want to play.
    ///
    /// Examples:
    ///   mate accept abc123
    ///   mate accept abc123 --color white
    ///   mate accept abc123 --color black
    Accept {
        /// Game ID of the invitation to accept
        game_id: String,
        /// Color preference: 'white', 'black', or 'random' (default: remaining color)
        #[arg(short, long)]
        color: Option<String>,
    },

    /// Make a chess move in a game
    ///
    /// Makes a move using standard algebraic notation (SAN).
    /// If no game ID is provided, makes the move in the most recently active game.
    ///
    /// Examples:
    ///   mate move e4
    ///   mate move Nf3
    ///   mate move O-O
    ///   mate move exd5
    ///   mate move Qh5#
    ///   mate move e4 --game-id abc123
    Move {
        /// The chess move in algebraic notation (e.g., e4, Nf3, O-O, Qxe7+)
        chess_move: String,
        /// Specific game ID to make the move in. If not provided, uses most recent game
        #[arg(short, long)]
        game_id: Option<String>,
    },

    /// Show move history for a chess game
    ///
    /// Displays the complete move history of a chess game in standard
    /// algebraic notation, along with game metadata.
    /// If no game ID is provided, shows history for the most recently active game.
    ///
    /// Examples:
    ///   mate history
    ///   mate history --game-id abc123
    History {
        /// Specific game ID to show history for. If not provided, shows most recent game
        #[arg(short, long)]
        game_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum KeyCommand {
    /// Show the default key storage path
    Path,
    /// Generate a new identity (overwrites existing)
    Generate,
    /// Show current identity info
    Info,
}
