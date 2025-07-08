use crate::chess::{Color, Move};
use crate::cli::game_ops::{GameOps, GameOpsError};
use crate::messages::chess::{validate_chess_move_format, validate_game_id};
use crate::storage::{Database, StorageError};
use std::io::{self, Write};
use std::net::{SocketAddr, ToSocketAddrs};

/// Comprehensive validation error for CLI operations
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid chess move: {0}")]
    InvalidMove(String),

    #[error("Invalid game ID: {0}")]
    InvalidGameId(String),

    #[error("Invalid peer address: {0}")]
    InvalidPeerAddress(String),

    #[error("Invalid color specification: {0}")]
    InvalidColor(String),

    #[error("Game not found: {0}")]
    GameNotFound(String),

    #[error("Multiple games found matching '{0}': {1}")]
    AmbiguousGameId(String, String),

    #[error("No active games found")]
    NoActiveGames,

    #[error("Operation cancelled by user")]
    UserCancelled,

    #[error("Database error: {0}")]
    Database(#[from] StorageError),

    #[error("Game operations error: {0}")]
    GameOps(#[from] GameOpsError),

    #[error("I/O Error: {0}")]
    Io(#[from] io::Error),
}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// CLI input validator with comprehensive validation and user experience features
pub struct InputValidator<'a> {
    game_ops: GameOps<'a>,
}

impl<'a> InputValidator<'a> {
    /// Create a new input validator
    pub fn new(database: &'a Database) -> Self {
        Self {
            game_ops: GameOps::new(database),
        }
    }

    /// Validate chess move notation with detailed error messages
    ///
    /// Supports standard algebraic notation including:
    /// - Basic moves: e2e4, d7d5
    /// - Promotion moves: e7e8q, a7a8n
    /// - Castling: O-O, O-O-O, 0-0, 0-0-0
    ///
    /// Returns user-friendly error messages with suggestions
    pub fn validate_chess_move(&self, move_str: &str) -> ValidationResult<String> {
        let trimmed = move_str.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::InvalidMove(
                "Move cannot be empty. Try moves like 'e4', 'Nf3', or 'O-O'".to_string(),
            ));
        }

        // Use existing chess message validation for format checking
        if let Err(e) = validate_chess_move_format(trimmed) {
            return Err(ValidationError::InvalidMove(
                format!("{e}\n\nValid move formats:\n  • Basic moves: e2e4, d7d5\n  • Promotion: e7e8q (queen), e7e8n (knight)\n  • Castling: O-O (kingside), O-O-O (queenside)")
            ));
        }

        // Additional validation for common user mistakes
        if trimmed.contains(" ") {
            return Err(ValidationError::InvalidMove(
                "Chess moves should not contain spaces. Use 'e2e4' instead of 'e2 e4'".to_string(),
            ));
        }

        if trimmed.contains("-") && !trimmed.starts_with("O-O") {
            return Err(ValidationError::InvalidMove(
                "Use algebraic notation without dashes. Use 'e2e4' instead of 'e2-e4'".to_string(),
            ));
        }

        // Check for common notation mistakes
        if trimmed.len() > 6 {
            return Err(ValidationError::InvalidMove(
                "Move notation is too long. Most moves are 4-5 characters (e2e4, e7e8q)"
                    .to_string(),
            ));
        }

        Ok(trimmed.to_string())
    }

    /// Validate and parse color specification
    ///
    /// Accepts: white, black, w, b, random, rand (case insensitive)
    /// Returns normalized color string or None for random
    pub fn validate_color(&self, color_str: &str) -> ValidationResult<Option<Color>> {
        let normalized = color_str.trim().to_lowercase();

        match normalized.as_str() {
            "white" | "w" => Ok(Some(Color::White)),
            "black" | "b" => Ok(Some(Color::Black)),
            "random" | "rand" | "r" => Ok(None),
            "" => Ok(None), // Empty string means random
            _ => Err(ValidationError::InvalidColor(format!(
                "Invalid color '{color_str}'. Use 'white', 'black', or 'random'"
            ))),
        }
    }

    /// Validate peer network address
    ///
    /// Accepts various formats:
    /// - IP:port (127.0.0.1:8080)
    /// - hostname:port (localhost:8080)
    /// - IPv6 addresses ([::1]:8080)
    pub fn validate_peer_address(&self, address: &str) -> ValidationResult<SocketAddr> {
        let trimmed = address.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::InvalidPeerAddress(
                "Address cannot be empty. Use format like '127.0.0.1:8080'".to_string(),
            ));
        }

        // Try to resolve the address
        match trimmed.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    Ok(addr)
                } else {
                    Err(ValidationError::InvalidPeerAddress(format!(
                        "Could not resolve address '{trimmed}'. Check the hostname and port"
                    )))
                }
            }
            Err(e) => {
                // Provide helpful error messages for common mistakes
                if !trimmed.contains(':') {
                    Err(ValidationError::InvalidPeerAddress(format!(
                        "Missing port number in '{trimmed}'. Use format like '127.0.0.1:8080'"
                    )))
                } else if trimmed.ends_with(':') {
                    Err(ValidationError::InvalidPeerAddress(format!(
                        "Missing port number after ':' in '{trimmed}'. Use format like '127.0.0.1:8080'"
                    )))
                } else {
                    Err(ValidationError::InvalidPeerAddress(format!(
                        "Invalid address '{trimmed}': {e}. Use format like '127.0.0.1:8080'"
                    )))
                }
            }
        }
    }

    /// Validate game ID with fuzzy matching support
    ///
    /// If exact match is found, returns the game ID
    /// If no exact match but fuzzy matches exist, returns suggestions
    /// Supports partial UUID matching (e.g., "abc123" matches "abc123-4567-...")
    pub fn validate_and_resolve_game_id(&self, input_id: &str) -> ValidationResult<String> {
        let trimmed = input_id.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::InvalidGameId(
                "Game ID cannot be empty".to_string(),
            ));
        }

        // Get all active games from game operations
        let games = self.game_ops.list_active_games()?;

        if games.is_empty() {
            return Err(ValidationError::NoActiveGames);
        }

        // First, try exact match
        for game_record in &games {
            if game_record.game.id == trimmed {
                return Ok(game_record.game.id.clone());
            }
        }

        // Try fuzzy matching - partial UUID match
        let mut partial_matches = Vec::new();
        let input_lower = trimmed.to_lowercase();

        for game_record in &games {
            let game_id_lower = game_record.game.id.to_lowercase();

            // Check if input is a prefix or appears anywhere in the game ID
            if game_id_lower.starts_with(&input_lower)
                || (game_id_lower.contains(&input_lower) && input_lower.len() >= 6)
            {
                partial_matches.push(game_record.game.id.clone());
            }
        }

        match partial_matches.len() {
            0 => Err(ValidationError::GameNotFound(format!(
                "No game found matching '{trimmed}'. Use 'mate games' to see available games"
            ))),
            1 => Ok(partial_matches[0].clone()),
            _ => {
                let suggestions = partial_matches.join(", ");
                Err(ValidationError::AmbiguousGameId(
                    trimmed.to_string(),
                    format!("Please be more specific. Matching games: {suggestions}"),
                ))
            }
        }
    }

    /// Get the most recent active game ID
    ///
    /// Used when no game ID is specified for board, move, or history commands
    pub fn get_most_recent_game_id(&self) -> ValidationResult<String> {
        let games = self.game_ops.list_active_games()?;

        if games.is_empty() {
            return Err(ValidationError::NoActiveGames);
        }

        // Find the most recently updated game
        let most_recent = games
            .iter()
            .max_by_key(|game_record| &game_record.game.updated_at)
            .unwrap(); // Safe because we checked games is not empty

        Ok(most_recent.game.id.clone())
    }

    /// Validate that a game ID represents a valid UUID format
    ///
    /// This is a stricter validation for game IDs that should be UUIDs
    pub fn validate_uuid_format(&self, game_id: &str) -> ValidationResult<String> {
        let trimmed = game_id.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::InvalidGameId(
                "Game ID cannot be empty".to_string(),
            ));
        }

        // Use existing game ID validation from messages module
        if !validate_game_id(trimmed) {
            return Err(ValidationError::InvalidGameId(format!(
                "'{trimmed}' is not a valid game ID format. Game IDs should be UUIDs"
            )));
        }

        Ok(trimmed.to_string())
    }

    /// Prompt user for confirmation of potentially destructive actions
    ///
    /// Returns true if user confirms, false if they decline
    /// Supports both y/n and yes/no responses (case insensitive)
    #[allow(clippy::only_used_in_recursion)]
    pub fn confirm_action(&self, prompt: &str) -> ValidationResult<bool> {
        print!("{} (y/n): ", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        match response.as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            "" => {
                // Empty response - ask again with default
                println!("Please enter 'y' for yes or 'n' for no.");
                self.confirm_action(prompt)
            }
            _ => {
                println!(
                    "Invalid response '{}'. Please enter 'y' for yes or 'n' for no.",
                    response
                );
                self.confirm_action(prompt)
            }
        }
    }

    /// Validate game ID or get most recent if None provided
    ///
    /// This is a convenience method for commands that accept optional game IDs
    pub fn resolve_game_id(&self, game_id: Option<&str>) -> ValidationResult<String> {
        match game_id {
            Some(id) => self.validate_and_resolve_game_id(id),
            None => self.get_most_recent_game_id(),
        }
    }

    /// Get suggestions for similar game IDs (for error messages)
    ///
    /// Used to provide helpful suggestions when game ID validation fails
    pub fn suggest_similar_game_ids(
        &self,
        input: &str,
        limit: usize,
    ) -> ValidationResult<Vec<String>> {
        let games = self.game_ops.list_active_games()?;

        if games.is_empty() {
            return Ok(vec![]);
        }

        let input_lower = input.to_lowercase();
        let mut suggestions = Vec::new();

        // Find games that contain the input as a substring
        for game_record in games {
            let game_id_lower = game_record.game.id.to_lowercase();
            if game_id_lower.contains(&input_lower) {
                suggestions.push(game_record.game.id);
            }
        }

        // Limit the number of suggestions
        suggestions.truncate(limit);
        Ok(suggestions)
    }

    /// Validate that a move is syntactically correct and try to parse it
    ///
    /// This performs deeper validation by attempting to parse the move
    /// with the chess engine, providing more specific error messages
    pub fn validate_and_parse_move(
        &self,
        move_str: &str,
        board_color: Color,
    ) -> ValidationResult<Move> {
        // First do basic format validation
        let validated_move = self.validate_chess_move(move_str)?;

        // Try to parse the move with the chess engine
        match Move::from_str_with_color(&validated_move, board_color) {
            Ok(chess_move) => Ok(chess_move),
            Err(e) => Err(ValidationError::InvalidMove(
                format!("Cannot parse move '{validated_move}': {e}\n\nTip: Use moves like 'e2e4', 'e7e8q' (promotion), or 'O-O' (castling)")
            ))
        }
    }
}

/// Utility functions for input validation that don't require database access
pub struct InputValidationUtils;

impl InputValidationUtils {
    /// Check if a string looks like a UUID (for quick validation)
    pub fn looks_like_uuid(s: &str) -> bool {
        // Basic UUID pattern check - 8-4-4-4-12 hexadecimal characters
        if s.len() != 36 {
            return false;
        }

        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 5 {
            return false;
        }

        let expected_lengths = [8, 4, 4, 4, 12];
        for (i, part) in parts.iter().enumerate() {
            if part.len() != expected_lengths[i] || !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return false;
            }
        }

        true
    }

    /// Normalize color input to standard form
    pub fn normalize_color_input(color: &str) -> String {
        match color.trim().to_lowercase().as_str() {
            "w" => "white".to_string(),
            "b" => "black".to_string(),
            "r" | "rand" => "random".to_string(),
            other => other.to_string(),
        }
    }

    /// Check if an address string has a valid format (basic syntax check)
    pub fn has_valid_address_format(address: &str) -> bool {
        let trimmed = address.trim();

        // Must contain a colon for port separation
        if !trimmed.contains(':') {
            return false;
        }

        // Must not start or end with colon
        if trimmed.starts_with(':') || trimmed.ends_with(':') {
            return false;
        }

        // Split by colon and check basic structure
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() < 2 || parts.len() > 8 {
            return false; // Allow for IPv6 addresses
        }

        // Last part should be a valid port number
        if let Some(port_str) = parts.last() {
            if let Ok(port) = port_str.parse::<u16>() {
                port > 0
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Generate a user-friendly error message for invalid moves
    pub fn move_error_with_suggestions(invalid_move: &str) -> String {
        let mut suggestions = Vec::new();

        // Common mistake patterns and suggestions
        if invalid_move.contains(" ") {
            suggestions.push("Remove spaces (use 'e2e4' not 'e2 e4')");
        }

        if invalid_move.contains("-") && !invalid_move.starts_with("O-O") {
            suggestions.push("Remove dashes (use 'e2e4' not 'e2-e4')");
        }

        if invalid_move.len() < 3 {
            suggestions.push("Move too short (try 'e2e4' or 'O-O')");
        }

        if invalid_move.len() > 6 {
            suggestions.push("Move too long (most moves are 4-5 characters)");
        }

        let mut message = format!("Invalid move '{invalid_move}'");
        if !suggestions.is_empty() {
            let suggestion_list = suggestions.join(", ");
            message.push_str(&format!("\nSuggestions: {suggestion_list}"));
        }

        message.push_str("\n\nValid formats:\n  • Basic: e2e4, d7d5\n  • Promotion: e7e8q\n  • Castling: O-O, O-O-O");
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;
    use tempfile::tempdir;

    // Helper function to create a test database
    fn create_test_database() -> Database {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        Database::new(db_path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn test_chess_move_validation() {
        let db = create_test_database();
        let validator = InputValidator::new(&db);

        // Valid moves
        assert!(validator.validate_chess_move("e2e4").is_ok());
        assert!(validator.validate_chess_move("e7e8q").is_ok());
        assert!(validator.validate_chess_move("O-O").is_ok());
        assert!(validator.validate_chess_move("O-O-O").is_ok());

        // Invalid moves
        assert!(validator.validate_chess_move("").is_err());
        assert!(validator.validate_chess_move("e2 e4").is_err());
        assert!(validator.validate_chess_move("e2-e4").is_err());
        assert!(validator.validate_chess_move("invalid").is_err());
    }

    #[test]
    fn test_color_validation() {
        let db = create_test_database();
        let validator = InputValidator::new(&db);

        // Valid colors
        assert_eq!(
            validator.validate_color("white").unwrap(),
            Some(Color::White)
        );
        assert_eq!(
            validator.validate_color("black").unwrap(),
            Some(Color::Black)
        );
        assert_eq!(validator.validate_color("w").unwrap(), Some(Color::White));
        assert_eq!(validator.validate_color("b").unwrap(), Some(Color::Black));
        assert_eq!(validator.validate_color("random").unwrap(), None);
        assert_eq!(validator.validate_color("").unwrap(), None);

        // Invalid colors
        assert!(validator.validate_color("red").is_err());
        assert!(validator.validate_color("invalid").is_err());
    }

    #[test]
    fn test_uuid_format_validation() {
        let db = create_test_database();
        let validator = InputValidator::new(&db);

        // Valid UUID
        let valid_uuid = "123e4567-e89b-12d3-a456-426614174000";
        assert!(validator.validate_uuid_format(valid_uuid).is_ok());

        // Invalid UUIDs
        assert!(validator.validate_uuid_format("").is_err());
        assert!(validator.validate_uuid_format("not-a-uuid").is_err());
        assert!(validator.validate_uuid_format("123-456").is_err());
    }

    #[test]
    fn test_address_format_validation() {
        assert!(InputValidationUtils::has_valid_address_format(
            "127.0.0.1:8080"
        ));
        assert!(InputValidationUtils::has_valid_address_format(
            "localhost:3000"
        ));
        assert!(InputValidationUtils::has_valid_address_format("[::1]:8080"));

        assert!(!InputValidationUtils::has_valid_address_format("127.0.0.1"));
        assert!(!InputValidationUtils::has_valid_address_format(":8080"));
        assert!(!InputValidationUtils::has_valid_address_format(
            "127.0.0.1:"
        ));
        assert!(!InputValidationUtils::has_valid_address_format(""));
    }

    #[test]
    fn test_uuid_recognition() {
        assert!(InputValidationUtils::looks_like_uuid(
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        assert!(!InputValidationUtils::looks_like_uuid("not-a-uuid"));
        assert!(!InputValidationUtils::looks_like_uuid("123-456"));
        assert!(!InputValidationUtils::looks_like_uuid(""));
    }
}
