use crate::chess::{Board, Color};
use crate::crypto::Identity;
use crate::messages::chess::Move as ChessMove;
use crate::storage::models::{GameStatus, PlayerColor};
use crate::storage::Database;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
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
        ProjectDirs::from("dev", "mate", "mate")
            .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))
    }

    /// Get the default config directory
    pub fn default_config_dir() -> Result<PathBuf> {
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
}

impl App {
    /// Create a new App instance with proper initialization
    pub async fn new() -> Result<Self> {
        // Load or create configuration
        let config =
            Config::load_or_create_default().context("Failed to initialize configuration")?;

        // Ensure data directory exists
        Self::ensure_data_dir(&config.data_dir).context("Failed to create data directory")?;

        // Load or generate identity
        let identity =
            Arc::new(Identity::load_or_generate().context("Failed to initialize identity")?);

        // Initialize database
        let database =
            Database::new(identity.peer_id().as_str()).context("Failed to initialize database")?;

        Ok(App {
            identity,
            database,
            config,
        })
    }

    /// Ensure data directory exists with proper permissions
    pub fn ensure_data_dir(data_dir: &PathBuf) -> Result<()> {
        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir).with_context(|| {
                format!("Failed to create data directory: {}", data_dir.display())
            })?;
        }

        // Verify directory is writable
        let test_file = data_dir.join(".write_test");
        std::fs::write(&test_file, "test")
            .with_context(|| format!("Data directory is not writable: {}", data_dir.display()))?;
        std::fs::remove_file(&test_file).context("Failed to clean up write test file")?;

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

    /// Handle the 'games' command - List active games with status information
    pub async fn handle_games(&self) -> Result<()> {
        let games = self
            .database
            .get_all_games()
            .context("Failed to retrieve games from database")?;

        if games.is_empty() {
            println!("No games found.");
            println!("Use 'mate invite <address>' to start a new game.");
            return Ok(());
        }

        // Display header
        println!("{}", "=".repeat(80));
        println!("{:^80}", "CHESS GAMES");
        println!("{}", "=".repeat(80));
        println!(
            "{:<12} {:<20} {:<8} {:<10} {:<15} {:<10}",
            "GAME ID", "OPPONENT", "COLOR", "STATUS", "LAST UPDATED", "RESULT"
        );
        println!("{}", "-".repeat(80));

        // Display each game
        for game in &games {
            let game_id_short = if game.id.len() > 8 {
                format!("{}...", &game.id[..8])
            } else {
                game.id.clone()
            };

            let opponent_short = if game.opponent_peer_id.len() > 16 {
                format!("{}...", &game.opponent_peer_id[..16])
            } else {
                game.opponent_peer_id.clone()
            };

            let color_str = match game.my_color {
                PlayerColor::White => "White",
                PlayerColor::Black => "Black",
            };

            let status_str = match game.status {
                GameStatus::Pending => "Pending",
                GameStatus::Active => "Active",
                GameStatus::Completed => "Completed",
                GameStatus::Abandoned => "Abandoned",
            };

            // Format timestamp (simple approach)
            let updated_time = format_timestamp(game.updated_at);

            let result_str = match &game.result {
                Some(result) => format!("{:?}", result),
                None => "-".to_string(),
            };

            println!(
                "{:<12} {:<20} {:<8} {:<10} {:<15} {:<10}",
                game_id_short, opponent_short, color_str, status_str, updated_time, result_str
            );
        }

        println!("{}", "-".repeat(80));
        println!("Total games: {}", games.len());
        println!();
        println!("Use 'mate board --game-id <id>' to view a specific game board.");
        println!("Use 'mate history --game-id <id>' to view game move history.");

        Ok(())
    }

    /// Handle the 'board' command - Show board for a game
    pub async fn handle_board(&self, game_id: Option<String>) -> Result<()> {
        // Determine which game to show
        let target_game_id = match game_id {
            Some(id) => id,
            None => {
                // Find the most recently active game
                let games = self
                    .database
                    .get_all_games()
                    .context("Failed to retrieve games from database")?;

                let active_game = games
                    .iter()
                    .find(|g| matches!(g.status, GameStatus::Active | GameStatus::Pending))
                    .or_else(|| games.first());

                match active_game {
                    Some(game) => game.id.clone(),
                    None => {
                        println!("No games found.");
                        println!("Use 'mate invite <address>' to start a new game.");
                        return Ok(());
                    }
                }
            }
        };

        // Get the game from database
        let game = self
            .database
            .get_game(&target_game_id)
            .context("Failed to retrieve game from database")?;

        // Get move history for the game
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
                // Parse the move message content
                match serde_json::from_str::<ChessMove>(&message.content) {
                    Ok(_move_msg) => {
                        // Parse algebraic notation and apply to board
                        // For now, we'll show a placeholder since move parsing is complex
                        move_count += 1;
                    }
                    Err(_) => {
                        // Skip invalid move messages
                        continue;
                    }
                }
            }
        }

        // Display game information
        println!("{}", "=".repeat(60));
        println!(
            "{:^60}",
            format!(
                "CHESS BOARD - GAME {}",
                if target_game_id.len() > 8 {
                    format!("{}...", &target_game_id[..8])
                } else {
                    target_game_id.clone()
                }
            )
        );
        println!("{}", "=".repeat(60));
        println!("Opponent: {}", game.opponent_peer_id);
        println!("Your Color: {:?}", game.my_color);
        println!("Status: {:?}", game.status);
        println!("Moves Played: {}", move_count);
        if let Some(result) = &game.result {
            println!("Result: {:?}", result);
        }
        println!("{}", "-".repeat(60));

        // Display the board
        println!("{}", board.to_ascii());

        println!("{}", "-".repeat(60));

        // Show whose turn it is
        let turn_color = board.active_color();
        let is_my_turn = match (turn_color, game.my_color) {
            (Color::White, PlayerColor::White) | (Color::Black, PlayerColor::Black) => true,
            _ => false,
        };

        if game.status == GameStatus::Active {
            if is_my_turn {
                println!("It's your turn to move!");
                println!("Use 'mate move <move>' to make a move (e.g., 'mate move e4')");
            } else {
                println!("Waiting for opponent's move...");
            }
        } else if game.status == GameStatus::Pending {
            println!("Game is pending - waiting for opponent to accept invitation.");
        }

        println!(
            "Use 'mate history --game-id {}' to see the complete move history.",
            target_game_id
        );

        Ok(())
    }
}

/// Format a Unix timestamp into a human-readable string
fn format_timestamp(timestamp: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    match UNIX_EPOCH.checked_add(std::time::Duration::from_secs(timestamp as u64)) {
        Some(time) => {
            let elapsed = SystemTime::now().duration_since(time).unwrap_or_default();

            if elapsed.as_secs() < 60 {
                "Just now".to_string()
            } else if elapsed.as_secs() < 3600 {
                format!("{}m ago", elapsed.as_secs() / 60)
            } else if elapsed.as_secs() < 86400 {
                format!("{}h ago", elapsed.as_secs() / 3600)
            } else {
                format!("{}d ago", elapsed.as_secs() / 86400)
            }
        }
        None => "Unknown".to_string(),
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
