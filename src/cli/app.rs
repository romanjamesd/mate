use crate::chess::{Board, Color};
use crate::cli::display::{
    display_board, display_game_status, display_games_list, display_move_history,
};
use crate::cli::network_manager::NetworkManager;
use crate::cli::validation::InputValidator;
use crate::crypto::Identity;
use crate::game::{store_game_invite_message, GameOps};
use crate::messages::chess::Move as ChessMove;
use crate::messages::chess::{generate_game_id, hash_board_state, GameAccept, GameInvite};
use crate::messages::types::Message;

use crate::storage::models::{GameStatus, PlayerColor};
use crate::storage::Database;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use std::sync::Arc;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory for storing application data
    pub data_dir: PathBuf,
    /// Default bind address for the server
    pub default_bind_addr: String,
    /// Maximum number of concurrent games
    pub max_concurrent_games: usize,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = Self::default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            data_dir,
            default_bind_addr: "127.0.0.1:8080".to_string(),
            max_concurrent_games: 10,
        }
    }
}

impl Config {
    /// Get the default data directory
    pub fn default_data_dir() -> Result<PathBuf> {
        // Check for test override environment variable first
        if let Ok(custom_data_dir) = std::env::var("MATE_DATA_DIR") {
            return Ok(PathBuf::from(custom_data_dir));
        }

        ProjectDirs::from("dev", "mate", "mate")
            .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))
    }

    /// Get the default config directory
    pub fn default_config_dir() -> Result<PathBuf> {
        // Check for test override environment variable first
        if let Ok(custom_config_dir) = std::env::var("MATE_CONFIG_DIR") {
            return Ok(PathBuf::from(custom_config_dir));
        }

        ProjectDirs::from("dev", "mate", "mate")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
    }

    /// Get the default config file path
    pub fn default_config_file() -> Result<PathBuf> {
        Ok(Self::default_config_dir()?.join("config.toml"))
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load_or_create_default() -> Result<Self> {
        let config_file = Self::default_config_file()?;

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)
                .context("Failed to read configuration file")?;
            let config: Config =
                toml::from_str(&content).context("Failed to parse configuration file")?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_file = Self::default_config_file()?;

        // Ensure config directory exists
        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&config_file, content).context("Failed to write configuration file")?;

        Ok(())
    }

    /// Get the database path
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("database.sqlite")
    }
}

/// Main application state
pub struct App {
    /// Cryptographic identity
    pub identity: Arc<Identity>,
    /// Database connection
    pub database: Database,
    /// Application configuration
    pub config: Config,
    /// Network manager for peer connections
    pub network_manager: NetworkManager,
}

impl App {
    /// Create a new App instance with proper initialization
    pub async fn new() -> Result<Self> {
        // Load or create configuration
        let config =
            Config::load_or_create_default().context("Failed to initialize configuration")?;

        Self::new_with_config(config).await
    }

    /// Create a new App instance with the given configuration
    pub async fn new_with_config(config: Config) -> Result<Self> {
        // Ensure data directory exists
        Self::ensure_data_dir(&config.data_dir).context("Failed to create data directory")?;

        // Load or generate identity using the specific data directory (no environment variables)
        let identity = Arc::new(
            Identity::load_or_generate_from_data_dir(&config.data_dir)
                .context("Failed to initialize identity")?,
        );

        // Initialize database with explicit path (no environment variables needed)
        let database =
            Database::new(identity.peer_id().as_str()).context("Failed to initialize database")?;

        // Initialize network manager
        let network_manager = NetworkManager::new(identity.clone());

        Ok(App {
            identity,
            database,
            config,
            network_manager,
        })
    }

    /// Create a new App instance with custom data directory (primarily for testing)
    pub async fn new_with_data_dir(data_dir: std::path::PathBuf) -> Result<Self> {
        // Create custom configuration with specified data directory
        let config = Config {
            data_dir: data_dir.clone(),
            default_bind_addr: "127.0.0.1:8080".to_string(),
            max_concurrent_games: 10,
        };

        // Ensure data directory exists
        Self::ensure_data_dir(&config.data_dir).context("Failed to create data directory")?;

        // Load or generate identity using the specific data directory (no environment variables)
        let identity = Arc::new(
            Identity::load_or_generate_from_data_dir(&data_dir)
                .context("Failed to initialize identity")?,
        );

        // Initialize database with explicit path (no environment variables needed)
        let db_path = data_dir.join("database.sqlite");
        let database = Database::new_with_path(identity.peer_id().as_str(), &db_path)
            .context("Failed to initialize database")?;

        // Initialize network manager
        let network_manager = NetworkManager::new(identity.clone());

        Ok(App {
            identity,
            database,
            config,
            network_manager,
        })
    }

    /// Ensure data directory exists with proper permissions
    pub fn ensure_data_dir(data_dir: &PathBuf) -> Result<()> {
        if !data_dir.exists() {
            let dir_display = data_dir.display();
            std::fs::create_dir_all(data_dir)
                .with_context(|| format!("Failed to create data directory: {dir_display}"))?;
        }

        // Verify directory is writable
        let test_file = data_dir.join(".write_test");
        let dir_display = data_dir.display();
        std::fs::write(&test_file, "test")
            .with_context(|| format!("Data directory is not writable: {dir_display}"))?;

        // Try to clean up test file, but don't fail if it can't be removed
        // (test directories might be cleaned up by test framework)
        let _ = std::fs::remove_file(&test_file);

        Ok(())
    }

    /// Get the peer ID
    pub fn peer_id(&self) -> &str {
        self.identity.peer_id().as_str()
    }

    /// Get the database path
    pub fn database_path(&self) -> PathBuf {
        self.config.database_path()
    }

    /// Get data directory path
    pub fn data_dir(&self) -> &PathBuf {
        &self.config.data_dir
    }

    /// Reload configuration from file
    pub fn reload_config(&mut self) -> Result<()> {
        self.config = Config::load_or_create_default().context("Failed to reload configuration")?;
        Ok(())
    }

    /// Save current configuration to file
    pub fn save_config(&self) -> Result<()> {
        self.config.save().context("Failed to save configuration")
    }

    /// Resolve a game id for read commands (`board` / `history`).
    ///
    /// With no id, picks the most recent Pending/Active game. With an id,
    /// matches any status (exact or unique prefix) so completed games remain viewable.
    fn resolve_read_game_id(&self, game_id: Option<&str>) -> Result<String> {
        let ops = GameOps::new(&self.database);
        match game_id {
            Some(id) => Ok(ops.find_game_by_partial_id(id)?.id),
            None => Ok(ops.get_current_game_id()?),
        }
    }

    /// Handle the 'games' command - List active games with status information
    pub async fn handle_games(&self) -> Result<()> {
        let records = GameOps::new(&self.database).list_games()?;
        display_games_list(&records);
        if records.is_empty() {
            println!("Use 'mate invite <address>' to start a new game.");
        }
        Ok(())
    }

    /// Handle the 'board' command - Show board for a game
    pub async fn handle_board(&self, game_id: Option<String>) -> Result<()> {
        let target_game_id = self.resolve_read_game_id(game_id.as_deref())?;
        let state = GameOps::new(&self.database).reconstruct_game_state(&target_game_id)?;

        let game_display = if target_game_id.len() > 8 {
            format!("{}...", &target_game_id[..8])
        } else {
            target_game_id.clone()
        };
        println!("Game: {game_display}");
        println!("Opponent: {}", state.game.opponent_peer_id);
        display_game_status(&state.game.status, state.game.result.as_ref());
        display_board(&state.board, Color::from(state.game.my_color));

        if state.game.status == GameStatus::Active {
            if state.your_turn {
                println!("It's your turn to move!");
                println!("Use 'mate move <move>' to make a move (e.g., 'mate move e2e4')");
            } else {
                println!("Waiting for opponent's move...");
            }
        }

        println!("Use 'mate history --game-id {target_game_id}' to see the complete move history.");

        Ok(())
    }

    /// Handle the 'invite' command - Send game invitation to a peer
    pub async fn handle_invite(&self, address: String, color: Option<String>) -> Result<()> {
        let validator = InputValidator::new(&self.database);
        validator
            .validate_peer_address(&address)
            .map_err(|e| anyhow::anyhow!(e))?;
        let address = address.trim().to_string();

        let suggested_color = match color.as_deref() {
            Some(c) => validator
                .validate_color(c)
                .map_err(|e| anyhow::anyhow!(e))?,
            None => None,
        };

        println!("Sending chess game invitation to {address}...");

        // Determine our color based on suggestion
        let my_color = match suggested_color {
            Some(Color::White) => PlayerColor::Black, // We suggested white for them, so we're black
            Some(Color::Black) => PlayerColor::White, // We suggested black for them, so we're white
            None => {
                // Random assignment - let's choose white for ourselves
                PlayerColor::White
            }
        };

        // Create the game record with dial address in metadata. Opponent peer
        // id is filled in after a successful handshake.
        let game = self
            .database
            .create_game_with_id(
                generate_game_id(),
                String::new(),
                my_color.clone(),
                Some(json!({ "dial_address": address })),
            )
            .context("Failed to create game record")?;

        let game_display = if game.id.len() > 8 {
            let short_id = &game.id[..8];
            format!("{short_id}...")
        } else {
            game.id.clone()
        };
        let game_full_id = &game.id;
        println!("Created game {game_display} with ID: {game_full_id}");

        // Create game invitation
        let invite = GameInvite::new(game.id.clone(), suggested_color);

        // Send the invitation using network manager
        match self
            .network_manager
            .send_game_invite(&address, game.id.clone(), invite.clone())
            .await
        {
            Ok(outcome) => {
                println!("✓ Invitation sent successfully!");

                if let Err(e) = self
                    .database
                    .update_opponent_peer_id(&game.id, &outcome.peer_id)
                {
                    eprintln!("Warning: Failed to update opponent peer id: {e}");
                }

                if let Err(e) =
                    store_game_invite_message(&self.database, &invite, "local", self.peer_id())
                {
                    eprintln!("Warning: Failed to store invitation message: {e}");
                }

                let game_id_str = &game.id;
                println!("Game ID: {game_id_str}");
                match suggested_color {
                    Some(Color::White) => println!("You will play as Black if they accept"),
                    Some(Color::Black) => println!("You will play as White if they accept"),
                    None => println!("Color will be determined when they accept"),
                }
                println!("Waiting for opponent to accept...");
                println!("Use 'mate games' to check invitation status.");

                // Log the response type for debugging
                match outcome.response {
                    Message::GameAccept(_) => {
                        println!("⚡ Invitation accepted immediately!");
                        // Update game status to active
                        if let Err(e) = self
                            .database
                            .update_game_status(&game.id, GameStatus::Active)
                        {
                            eprintln!("Warning: Failed to update game status: {e}");
                        }
                    }
                    Message::GameDecline(_) => {
                        println!("❌ Invitation declined.");
                        // Update game status to abandoned
                        if let Err(e) = self
                            .database
                            .update_game_status(&game.id, GameStatus::Abandoned)
                        {
                            eprintln!("Warning: Failed to update game status: {e}");
                        }
                    }
                    _ => {
                        // Other response types - invitation is pending
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to send invitation: {e}");
                // Update game status to abandoned since we couldn't send
                if let Err(db_err) = self
                    .database
                    .update_game_status(&game.id, GameStatus::Abandoned)
                {
                    eprintln!("Warning: Failed to update game status: {db_err}");
                }
                anyhow::bail!("Could not send invitation to {address}: {e}");
            }
        }

        Ok(())
    }

    /// Handle the 'accept' command - Accept a pending game invitation
    pub async fn handle_accept(&self, game_id: String, color: Option<String>) -> Result<()> {
        let game_display = if game_id.len() > 8 {
            let short_id = &game_id[..8];
            format!("{short_id}...")
        } else {
            game_id.clone()
        };
        println!("Accepting game invitation {game_display}...");

        // Validate game ID exists and is pending
        let game = self.database.get_game(&game_id).context("Game not found")?;

        if game.status != GameStatus::Pending {
            let current_status = game.status;
            anyhow::bail!("Game {game_id} is not in pending status (current: {current_status:?})");
        }

        // Parse color preference
        let accepted_color = match color.as_deref() {
            Some("white") => Color::White,
            Some("black") => Color::Black,
            Some("random") | None => {
                // Choose the opposite of what we have in the database
                match game.my_color {
                    PlayerColor::White => Color::Black,
                    PlayerColor::Black => Color::White,
                }
            }
            Some(invalid) => {
                anyhow::bail!(
                    "Invalid color '{}'. Use 'white', 'black', or 'random'",
                    invalid
                );
            }
        };

        // Update our color preference in the database if needed
        let _final_my_color = match accepted_color {
            Color::White => PlayerColor::White,
            Color::Black => PlayerColor::Black,
        };

        // Create game acceptance
        let accept = GameAccept::new(game_id.clone(), accepted_color);

        // Send the acceptance using network manager
        match self
            .network_manager
            .send_game_accept(&game.opponent_peer_id, game_id.clone(), accept)
            .await
        {
            Ok(_outcome) => {
                println!("✓ Game accepted successfully!");

                // Update game status to active
                self.database
                    .update_game_status(&game_id, GameStatus::Active)
                    .context("Failed to update game status to active")?;

                // Store the acceptance message in database
                if let Err(e) = self.database.store_message(
                    game_id.clone(),
                    "game_accept".to_string(),
                    serde_json::to_string(&GameAccept::new(game_id.clone(), accepted_color))
                        .unwrap_or_default(),
                    "local".to_string(), // Placeholder signature for sent messages
                    self.peer_id().to_string(),
                ) {
                    eprintln!("Warning: Failed to store acceptance message: {}", e);
                }

                println!(
                    "Game {} is now active!",
                    if game_id.len() > 8 {
                        let truncated = &game_id[..8];
                        format!("{truncated}...")
                    } else {
                        game_id.clone()
                    }
                );
                println!("You are playing as: {accepted_color:?}");

                // Show if it's our turn to move
                if accepted_color == Color::White {
                    println!(
                        "It's your turn to move! Use 'mate move <move>' to make your first move."
                    );
                } else {
                    println!("Waiting for opponent to make the first move...");
                }

                println!("Use 'mate board --game-id {game_id}' to view the board.");
            }
            Err(e) => {
                eprintln!("❌ Failed to send acceptance: {}", e);
                anyhow::bail!("Could not send acceptance: {}", e);
            }
        }

        Ok(())
    }

    /// Handle the 'move' command - Make a chess move in a game
    pub async fn handle_move(&self, game_id: Option<String>, chess_move: String) -> Result<()> {
        // Determine which game to make the move in
        let target_game_id = match game_id {
            Some(id) => id,
            None => {
                // Find the most recently active game
                let games = self
                    .database
                    .get_all_games()
                    .context("Failed to retrieve games from database")?;

                let active_game = games.iter().find(|g| g.status == GameStatus::Active);

                match active_game {
                    Some(game) => game.id.clone(),
                    None => {
                        anyhow::bail!("No active games found. Use --game-id to specify a game or start a new game with 'mate invite <address>'");
                    }
                }
            }
        };

        println!(
            "Making move '{}' in game {}...",
            chess_move,
            if target_game_id.len() > 8 {
                let truncated = &target_game_id[..8];
                format!("{truncated}...")
            } else {
                target_game_id.clone()
            }
        );

        // Get the game from database
        let game = self
            .database
            .get_game(&target_game_id)
            .context("Game not found")?;

        if game.status != GameStatus::Active {
            anyhow::bail!(
                "Game {target_game_id} is not active (current status: {status:?})",
                target_game_id = target_game_id,
                status = game.status
            );
        }

        // Get move history to reconstruct current board state
        let messages = self
            .database
            .get_messages_for_game(&target_game_id)
            .context("Failed to retrieve game messages")?;

        // Reconstruct board state from move history
        let board = Board::new(); // Start with initial position
        let mut move_count = 0;

        // Apply moves from message history
        for message in &messages {
            if message.message_type == "move" {
                // Parse the move message content and apply to board
                // For now, we'll increment move count and skip actual board updates
                // since implementing full move parsing is complex
                move_count += 1;
            }
        }

        // Check if it's our turn
        let current_turn = if move_count % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        let is_our_turn = matches!(
            (current_turn, &game.my_color),
            (Color::White, PlayerColor::White) | (Color::Black, PlayerColor::Black)
        );

        if !is_our_turn {
            anyhow::bail!(
                "It's not your turn to move. Current turn: {current_turn:?}, Your color: {my_color:?}",
                current_turn = current_turn,
                my_color = game.my_color
            );
        }

        // Validate move format (basic validation)
        if chess_move.trim().is_empty() {
            anyhow::bail!("Move cannot be empty");
        }

        // For now, we'll accept any non-empty move string
        // In a full implementation, we would:
        // 1. Parse the algebraic notation
        // 2. Validate it's a legal move on the current board
        // 3. Apply the move to get the new board state

        // Create board state hash (using current board for now)
        let board_hash = hash_board_state(&board);

        // Create chess move
        let chess_move_msg = ChessMove::new(
            target_game_id.clone(),
            chess_move.clone(),
            board_hash.clone(),
        );

        // Send the move using network manager
        match self
            .network_manager
            .send_chess_move(
                &game.opponent_peer_id,
                target_game_id.clone(),
                chess_move_msg,
            )
            .await
        {
            Ok(_outcome) => {
                println!("✓ Move '{}' sent successfully!", chess_move);

                // Store the move message in database
                if let Err(e) = self.database.store_message(
                    target_game_id.clone(),
                    "move".to_string(),
                    serde_json::to_string(&ChessMove::new(
                        target_game_id.clone(),
                        chess_move.clone(),
                        board_hash,
                    ))
                    .unwrap_or_default(),
                    "local".to_string(), // Placeholder signature for sent messages
                    self.peer_id().to_string(),
                ) {
                    eprintln!("Warning: Failed to store move message: {}", e);
                }

                println!("Waiting for opponent's response...");
                println!(
                    "Use 'mate board --game-id {}' to view the updated board.",
                    target_game_id
                );
                println!(
                    "Use 'mate history --game-id {}' to see the move history.",
                    target_game_id
                );
            }
            Err(e) => {
                eprintln!("❌ Failed to send move: {}", e);
                anyhow::bail!("Could not send move to opponent: {}", e);
            }
        }

        Ok(())
    }

    /// Handle the 'history' command - Show move history for a game
    pub async fn handle_history(&self, game_id: Option<String>) -> Result<()> {
        let target_game_id = self.resolve_read_game_id(game_id.as_deref())?;
        let state = GameOps::new(&self.database).reconstruct_game_state(&target_game_id)?;

        let game_display = if target_game_id.len() > 8 {
            format!("{}...", &target_game_id[..8])
        } else {
            target_game_id.clone()
        };
        println!("Game: {game_display}");
        println!("Opponent: {}", state.game.opponent_peer_id);
        display_game_status(&state.game.status, state.game.result.as_ref());
        display_move_history(&state.move_history, state.move_history.len() as u32);

        if state.game.status == GameStatus::Active {
            if state.your_turn {
                println!("It's your turn to move!");
                println!(
                    "Use 'mate move <move> --game-id {target_game_id}' to make your next move."
                );
            } else {
                println!("Waiting for opponent's move...");
            }
        }

        println!("Use 'mate board --game-id {target_game_id}' to view the current board position.");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.default_bind_addr, "127.0.0.1:8080");
        assert_eq!(config.max_concurrent_games, 10);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(config.default_bind_addr, deserialized.default_bind_addr);
        assert_eq!(
            config.max_concurrent_games,
            deserialized.max_concurrent_games
        );
    }

    #[test]
    fn test_ensure_data_dir() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("mate_data");

        // Directory doesn't exist yet
        assert!(!data_dir.exists());

        // Should create directory
        App::ensure_data_dir(&data_dir).unwrap();
        assert!(data_dir.exists());

        // Should work if directory already exists
        App::ensure_data_dir(&data_dir).unwrap();
    }
}
