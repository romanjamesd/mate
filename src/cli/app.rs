use crate::chess::{Board, Color};
use crate::crypto::Identity;
use crate::messages::chess::Move as ChessMove;
use crate::messages::chess::{hash_board_state, GameAccept, GameInvite};
use crate::messages::types::Message;
use crate::network::Client;
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

    /// Handle the 'invite' command - Send game invitation to a peer
    pub async fn handle_invite(&self, address: String, color: Option<String>) -> Result<()> {
        println!("Sending chess game invitation to {}...", address);

        // Parse color preference
        let suggested_color = match color.as_deref() {
            Some("white") => Some(Color::White),
            Some("black") => Some(Color::Black),
            Some("random") | None => None,
            Some(invalid) => {
                anyhow::bail!(
                    "Invalid color '{}'. Use 'white', 'black', or 'random'",
                    invalid
                );
            }
        };

        // Determine our color based on suggestion
        let my_color = match suggested_color {
            Some(Color::White) => PlayerColor::Black, // We suggested white for them, so we're black
            Some(Color::Black) => PlayerColor::White, // We suggested black for them, so we're white
            None => {
                // Random assignment - let's choose white for ourselves
                PlayerColor::White
            }
        };

        // Create the game record in database
        let game = self
            .database
            .create_game(
                address.clone(),
                my_color.clone(),
                None, // No metadata for now
            )
            .context("Failed to create game record")?;

        println!(
            "Created game {} with ID: {}",
            if game.id.len() > 8 {
                format!("{}...", &game.id[..8])
            } else {
                game.id.clone()
            },
            game.id
        );

        // Create network client
        let client = Client::new(self.identity.clone());

        // Create game invitation message
        let invite_message = Message::new_game_invite(game.id.clone(), suggested_color);

        // Send the invitation
        match client.send_message_to(&address, invite_message).await {
            Ok(response) => {
                println!("✓ Invitation sent successfully!");

                // Store the invitation message in database
                if let Err(e) = self.database.store_message(
                    game.id.clone(),
                    "game_invite".to_string(),
                    serde_json::to_string(&GameInvite::new(game.id.clone(), suggested_color))
                        .unwrap_or_default(),
                    "local".to_string(), // Placeholder signature for sent messages
                    self.peer_id().to_string(),
                ) {
                    eprintln!("Warning: Failed to store invitation message: {}", e);
                }

                println!("Game ID: {}", game.id);
                match suggested_color {
                    Some(Color::White) => println!("You will play as Black if they accept"),
                    Some(Color::Black) => println!("You will play as White if they accept"),
                    None => println!("Color will be determined when they accept"),
                }
                println!("Waiting for opponent to accept...");
                println!("Use 'mate games' to check invitation status.");

                // Log the response type for debugging
                match response {
                    Message::GameAccept(_) => {
                        println!("⚡ Invitation accepted immediately!");
                        // Update game status to active
                        if let Err(e) = self
                            .database
                            .update_game_status(&game.id, GameStatus::Active)
                        {
                            eprintln!("Warning: Failed to update game status: {}", e);
                        }
                    }
                    Message::GameDecline(_) => {
                        println!("❌ Invitation declined.");
                        // Update game status to abandoned
                        if let Err(e) = self
                            .database
                            .update_game_status(&game.id, GameStatus::Abandoned)
                        {
                            eprintln!("Warning: Failed to update game status: {}", e);
                        }
                    }
                    _ => {
                        // Other response types - invitation is pending
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to send invitation: {}", e);
                // Update game status to abandoned since we couldn't send
                if let Err(db_err) = self
                    .database
                    .update_game_status(&game.id, GameStatus::Abandoned)
                {
                    eprintln!("Warning: Failed to update game status: {}", db_err);
                }
                anyhow::bail!("Could not send invitation to {}: {}", address, e);
            }
        }

        Ok(())
    }

    /// Handle the 'accept' command - Accept a pending game invitation
    pub async fn handle_accept(&self, game_id: String, color: Option<String>) -> Result<()> {
        println!(
            "Accepting game invitation {}...",
            if game_id.len() > 8 {
                format!("{}...", &game_id[..8])
            } else {
                game_id.clone()
            }
        );

        // Validate game ID exists and is pending
        let game = self.database.get_game(&game_id).context("Game not found")?;

        if game.status != GameStatus::Pending {
            anyhow::bail!(
                "Game {} is not in pending status (current: {:?})",
                game_id,
                game.status
            );
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

        // Create network client
        let client = Client::new(self.identity.clone());

        // Create game acceptance message
        let accept_message = Message::new_game_accept(game_id.clone(), accepted_color);

        // Send the acceptance
        match client
            .send_message_to(&game.opponent_peer_id, accept_message)
            .await
        {
            Ok(_response) => {
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
                        format!("{}...", &game_id[..8])
                    } else {
                        game_id.clone()
                    }
                );
                println!("You are playing as: {:?}", accepted_color);

                // Show if it's our turn to move
                if accepted_color == Color::White {
                    println!(
                        "It's your turn to move! Use 'mate move <move>' to make your first move."
                    );
                } else {
                    println!("Waiting for opponent to make the first move...");
                }

                println!("Use 'mate board --game-id {}' to view the board.", game_id);
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
                format!("{}...", &target_game_id[..8])
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
                "Game {} is not active (current status: {:?})",
                target_game_id,
                game.status
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
        let is_our_turn = match (current_turn, &game.my_color) {
            (Color::White, PlayerColor::White) | (Color::Black, PlayerColor::Black) => true,
            _ => false,
        };

        if !is_our_turn {
            anyhow::bail!(
                "It's not your turn to move. Current turn: {:?}, Your color: {:?}",
                current_turn,
                game.my_color
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

        // Create network client
        let client = Client::new(self.identity.clone());

        // Create move message
        let move_message = Message::new_move(
            target_game_id.clone(),
            chess_move.clone(),
            board_hash.clone(),
        );

        // Send the move
        match client
            .send_message_to(&game.opponent_peer_id, move_message)
            .await
        {
            Ok(_response) => {
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
        // Determine which game to show history for
        let target_game_id = match game_id {
            Some(id) => id,
            None => {
                // Find the most recently active game
                let games = self
                    .database
                    .get_all_games()
                    .context("Failed to retrieve games from database")?;

                let recent_game = games
                    .iter()
                    .find(|g| matches!(g.status, GameStatus::Active | GameStatus::Completed))
                    .or_else(|| games.first());

                match recent_game {
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
            .context("Game not found")?;

        // Get move history for the game
        let messages = self
            .database
            .get_messages_for_game(&target_game_id)
            .context("Failed to retrieve game messages")?;

        // Filter for move messages
        let moves: Vec<_> = messages
            .iter()
            .filter(|m| m.message_type == "move")
            .collect();

        // Display game header
        println!("{}", "=".repeat(70));
        println!(
            "{:^70}",
            format!(
                "MOVE HISTORY - GAME {}",
                if target_game_id.len() > 8 {
                    format!("{}...", &target_game_id[..8])
                } else {
                    target_game_id.clone()
                }
            )
        );
        println!("{}", "=".repeat(70));

        // Display game metadata
        println!("Game ID: {}", target_game_id);
        println!("Opponent: {}", game.opponent_peer_id);
        println!("Your Color: {:?}", game.my_color);
        println!("Status: {:?}", game.status);
        if let Some(result) = &game.result {
            println!("Result: {:?}", result);
        }
        println!("Created: {}", format_timestamp(game.created_at));
        if let Some(completed_at) = game.completed_at {
            println!("Completed: {}", format_timestamp(completed_at));
        }
        println!("{}", "-".repeat(70));

        if moves.is_empty() {
            println!("No moves have been made in this game yet.");
            if game.status == GameStatus::Active {
                println!("Use 'mate move <move>' to make the first move!");
            }
        } else {
            println!("Moves:");
            println!(
                "{:<4} {:<12} {:<15} {:<20} {:<15}",
                "№", "MOVE", "PLAYER", "TIMESTAMP", "NOTATION"
            );
            println!("{}", "-".repeat(70));

            for (index, message) in moves.iter().enumerate() {
                let move_number = index + 1;
                let player = if message.sender_peer_id == self.peer_id() {
                    "You"
                } else {
                    "Opponent"
                };
                let timestamp = format_timestamp(message.created_at);

                // Try to parse the move content
                let move_notation = match serde_json::from_str::<ChessMove>(&message.content) {
                    Ok(chess_move) => chess_move.chess_move,
                    Err(_) => "Invalid".to_string(),
                };

                println!(
                    "{:<4} {:<12} {:<15} {:<20} {:<15}",
                    move_number,
                    move_notation,
                    player,
                    timestamp,
                    "-" // Placeholder for standard notation
                );
            }
        }

        println!("{}", "-".repeat(70));
        println!("Total moves: {}", moves.len());

        if game.status == GameStatus::Active {
            let current_turn = if moves.len() % 2 == 0 {
                Color::White
            } else {
                Color::Black
            };
            let is_our_turn = match (current_turn, &game.my_color) {
                (Color::White, PlayerColor::White) | (Color::Black, PlayerColor::Black) => true,
                _ => false,
            };

            if is_our_turn {
                println!("It's your turn to move!");
                println!(
                    "Use 'mate move <move> --game-id {}' to make your next move.",
                    target_game_id
                );
            } else {
                println!("Waiting for opponent's move...");
            }
        }

        println!(
            "Use 'mate board --game-id {}' to view the current board position.",
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
