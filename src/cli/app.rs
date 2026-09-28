use crate::chess::Color;
use crate::cli::display::{
    display_board, display_game_status, display_games_list, display_move_history,
};
use crate::cli::error_handler::{cli_error_from_anyhow, CliError, CliResult};
use crate::cli::network_manager::{GameAcceptOutcome, NetworkManager};
use crate::cli::validation::{InputValidationUtils, InputValidator};
use crate::crypto::Identity;
use crate::game::{
    store_game_accept_message, store_game_invite_message, GameOps, GameOpsError, MoveProcessor,
};
use crate::messages::chess::{generate_game_id, ChessProtocolError, GameAccept, GameInvite};
use crate::messages::types::Message;

use crate::storage::models::{Game, GameStatus, PlayerColor};
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
    fn resolve_read_game_id(&self, game_id: Option<&str>) -> CliResult<String> {
        let ops = GameOps::new(&self.database);
        match game_id {
            Some(id) => Ok(ops.find_game_by_partial_id(id)?.id),
            None => Ok(ops.get_current_game_id()?),
        }
    }

    /// Handle the 'games' command - List active games with status information
    pub async fn handle_games(&self) -> CliResult<()> {
        let records = GameOps::new(&self.database).list_games()?;
        display_games_list(&records);
        if records.is_empty() {
            println!("Use 'mate invite <address>' to start a new game.");
        }
        Ok(())
    }

    /// Handle the 'board' command - Show board for a game
    pub async fn handle_board(&self, game_id: Option<String>) -> CliResult<()> {
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
    pub async fn handle_invite(&self, address: String, color: Option<String>) -> CliResult<()> {
        let validator = InputValidator::new(&self.database);
        validator.validate_peer_address(&address)?;
        let address = address.trim().to_string();

        let suggested_color = match color.as_deref() {
            Some(c) => validator.validate_color(c)?,
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
            .map_err(CliError::from)?;

        let game_display = if game.id.len() > 8 {
            let short_id = &game.id[..8];
            format!("{short_id}...")
        } else {
            game.id.clone()
        };
        let game_full_id = &game.id;
        println!("Created game {game_display} with ID: {game_full_id}");

        // Create game invitation with dial-back listen address for the invitee.
        let invite = GameInvite::new(game.id.clone(), suggested_color)
            .with_reply_to(self.config.default_bind_addr.clone());

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
                // Update game status to abandoned since we couldn't send
                if let Err(db_err) = self
                    .database
                    .update_game_status(&game.id, GameStatus::Abandoned)
                {
                    eprintln!("Warning: Failed to update game status: {db_err}");
                }
                return Err(cli_error_from_anyhow(e));
            }
        }

        Ok(())
    }

    /// Handle the 'accept' command - Accept a pending game invitation
    pub async fn handle_accept(&self, game_id: String, color: Option<String>) -> CliResult<()> {
        let game_display = if game_id.len() > 8 {
            let short_id = &game_id[..8];
            format!("{short_id}...")
        } else {
            game_id.clone()
        };
        println!("Accepting game invitation {game_display}...");

        // Validate game ID exists and is pending
        let game = self.database.get_game(&game_id).map_err(CliError::from)?;

        if game.status != GameStatus::Pending {
            let current_status = game.status;
            return Err(CliError::GameOps(GameOpsError::InvalidGameState(format!(
                "Game {game_id} is not in pending status (current: {current_status:?})"
            ))));
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
                return Err(CliError::InvalidInput {
                    field: "color".to_string(),
                    value: invalid.to_string(),
                    reason: "Invalid color specification".to_string(),
                    suggestion: "Use 'white', 'black', or 'random'.".to_string(),
                });
            }
        };

        let my_color = match accepted_color {
            Color::White => PlayerColor::White,
            Color::Black => PlayerColor::Black,
        };

        let dial = resolve_dial_target(&game)?;
        let accept = GameAccept::new(game_id.clone(), accepted_color);

        let outcome = self
            .network_manager
            .send_game_accept(&dial, game_id.clone(), accept.clone())
            .await
            .map_err(cli_error_from_anyhow)?;

        if game.opponent_peer_id.is_empty() || outcome.peer_id() != game.opponent_peer_id {
            return Err(CliError::Protocol(ChessProtocolError::SecurityViolation {
                game_id,
                violation: format!(
                    "Acceptance response peer {} does not match game opponent {}",
                    outcome.peer_id(),
                    game.opponent_peer_id
                ),
            }));
        }

        match outcome {
            GameAcceptOutcome::Accepted {
                acknowledgement, ..
            } => {
                self.database
                    .update_game_status(&game_id, GameStatus::Active)
                    .map_err(CliError::from)?;

                self.database
                    .update_game_color(&game_id, my_color)
                    .map_err(CliError::from)?;

                store_game_accept_message(
                    &self.database,
                    &acknowledgement,
                    "local",
                    self.peer_id(),
                )
                .map_err(CliError::from)?;

                println!("✓ Game accepted successfully!");
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

                if accepted_color == Color::White {
                    println!(
                        "It's your turn to move! Use 'mate move <move>' to make your first move."
                    );
                } else {
                    println!("Waiting for opponent to make the first move...");
                }

                println!("Use 'mate board --game-id {game_id}' to view the board.");
            }
            GameAcceptOutcome::Rejected { decline, .. } => {
                return Err(CliError::GameAcceptanceRejected {
                    reason: decline.reason,
                });
            }
        }

        Ok(())
    }

    /// Handle the 'move' command - Make a chess move in a game
    pub async fn handle_move(&self, game_id: Option<String>, chess_move: String) -> CliResult<()> {
        let target_game_id = self.resolve_read_game_id(game_id.as_deref())?;

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

        let processor = MoveProcessor::new(&self.database);
        let prepared = processor.prepare_move(&target_game_id, &chess_move, true)?;

        let game = self
            .database
            .get_game(&target_game_id)
            .map_err(CliError::from)?;

        let dial = resolve_dial_target(&game)?;

        match self
            .network_manager
            .send_chess_move(&dial, target_game_id.clone(), prepared.move_message.clone())
            .await
        {
            Ok(_outcome) => {
                processor
                    .commit_move(
                        &target_game_id,
                        &prepared.move_message,
                        self.peer_id(),
                        "local",
                    )
                    .map_err(|e| CliError::UserError {
                        message: format!("Move sent but failed to persist locally: {e}"),
                        suggestion: Some(
                            "The move may have reached your opponent but local storage failed. Check database permissions and try 'mate board' to verify state.".to_string(),
                        ),
                    })?;

                println!("✓ Move '{}' sent successfully!", chess_move);
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
            Err(e) => return Err(cli_error_from_anyhow(e)),
        }

        Ok(())
    }

    /// Handle the 'history' command - Show move history for a game
    pub async fn handle_history(&self, game_id: Option<String>) -> CliResult<()> {
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

/// Resolve a dialable TCP address for outbound chess sends.
///
/// Prefers `metadata.dial_address`, then falls back to `opponent_peer_id` only
/// when it looks like `host:port`. Never dials a raw crypto peer id.
pub(crate) fn resolve_dial_target(game: &Game) -> CliResult<String> {
    if let Some(metadata) = &game.metadata {
        if let Some(addr) = metadata.get("dial_address").and_then(|v| v.as_str()) {
            let trimmed = addr.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    if InputValidationUtils::has_valid_address_format(&game.opponent_peer_id) {
        return Ok(game.opponent_peer_id.clone());
    }

    Err(CliError::UserError {
        message: format!(
            "No dialable address for game {}. Expected metadata dial_address (host:port).",
            game.id
        ),
        suggestion: Some(
            "Ensure the game has a dial_address in metadata from a successful invite handshake."
                .to_string(),
        ),
    })
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

    fn sample_game(opponent_peer_id: &str, metadata: Option<serde_json::Value>) -> Game {
        Game {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            opponent_peer_id: opponent_peer_id.to_string(),
            my_color: PlayerColor::White,
            status: GameStatus::Pending,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
            result: None,
            metadata,
        }
    }

    #[test]
    fn resolve_dial_target_prefers_metadata() {
        let game = sample_game(
            "crypto-peer-id-not-an-address",
            Some(json!({ "dial_address": "127.0.0.1:9000" })),
        );
        assert_eq!(resolve_dial_target(&game).unwrap(), "127.0.0.1:9000");
    }

    #[test]
    fn resolve_dial_target_falls_back_to_host_port_peer_id() {
        let game = sample_game("192.168.1.10:8080", None);
        assert_eq!(resolve_dial_target(&game).unwrap(), "192.168.1.10:8080");
    }

    #[test]
    fn resolve_dial_target_rejects_raw_peer_id() {
        let game = sample_game("abcdef0123456789abcdef0123456789abcdef01", None);
        let err = resolve_dial_target(&game).unwrap_err().to_string();
        assert!(err.contains("No dialable address"));
    }
}
