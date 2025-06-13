use super::moves::Move;
use super::{ChessError, Color, Piece, PieceType, Position};

/// Represents a chess board with piece positions and game state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// 8x8 array representing the chess board squares
    /// squares[rank][file] where rank 0 = rank 1, file 0 = file a
    squares: [[Option<Piece>; 8]; 8],

    /// Current player to move (White or Black)
    active_color: Color,

    /// Move counter (increments after Black's move)
    fullmove_number: u16,

    /// Halfmove counter for 50-move rule (resets on pawn moves and captures)
    halfmove_clock: u16,
}

impl Board {
    /// Create a new board with the standard starting position
    pub fn new() -> Self {
        let mut board = Self {
            squares: [[None; 8]; 8],
            active_color: Color::White,
            fullmove_number: 1,
            halfmove_clock: 0,
        };

        // Set up starting position
        board.setup_starting_position();
        board
    }

    /// Get the piece at the specified position, if any
    pub fn get_piece(&self, pos: Position) -> Option<Piece> {
        // Validate position bounds
        if pos.file > 7 || pos.rank > 7 {
            return None;
        }

        self.squares[pos.rank as usize][pos.file as usize]
    }

    /// Set a piece at the specified position
    pub fn set_piece(&mut self, pos: Position, piece: Option<Piece>) -> Result<(), ChessError> {
        // Validate position bounds
        if pos.file > 7 || pos.rank > 7 {
            return Err(ChessError::InvalidPosition(format!(
                "Position {}({},{}) is out of bounds",
                pos, pos.file, pos.rank
            )));
        }

        self.squares[pos.rank as usize][pos.file as usize] = piece;
        Ok(())
    }

    /// Get the current active color (player to move)
    pub fn active_color(&self) -> Color {
        self.active_color
    }

    /// Get the current fullmove number
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    /// Get the current halfmove clock
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    /// Set up the standard chess starting position
    fn setup_starting_position(&mut self) {
        // Clear the board first
        self.squares = [[None; 8]; 8];

        // Set up white pieces (rank 0 and 1)
        let white_back_rank = [
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
            PieceType::Bishop,
            PieceType::Knight,
            PieceType::Rook,
        ];

        for (file, &piece_type) in white_back_rank.iter().enumerate() {
            self.squares[0][file] = Some(Piece::new(piece_type, Color::White));
        }

        // White pawns on rank 1
        for file in 0..8 {
            self.squares[1][file] = Some(Piece::new(PieceType::Pawn, Color::White));
        }

        // Set up black pieces (rank 6 and 7)
        let black_back_rank = [
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
            PieceType::Bishop,
            PieceType::Knight,
            PieceType::Rook,
        ];

        for (file, &piece_type) in black_back_rank.iter().enumerate() {
            self.squares[7][file] = Some(Piece::new(piece_type, Color::Black));
        }

        // Black pawns on rank 6
        for file in 0..8 {
            self.squares[6][file] = Some(Piece::new(PieceType::Pawn, Color::Black));
        }
    }

    /// Create a board from a FEN (Forsyth-Edwards Notation) string
    /// FEN format: piece_placement active_color castling_rights en_passant halfmove fullmove
    /// Example: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    pub fn from_fen(fen: &str) -> Result<Board, ChessError> {
        // Handle empty or whitespace-only FEN strings
        let fen = fen.trim();
        if fen.is_empty() {
            return Err(ChessError::InvalidFen(
                "FEN string cannot be empty".to_string(),
            ));
        }

        // Step 4.2: Split and validate FEN fields
        let parts: Vec<&str> = fen.split_whitespace().collect();

        // Validate exactly 6 fields are present
        if parts.len() != 6 {
            return Err(ChessError::InvalidFen(format!(
                "FEN must have exactly 6 fields (piece_placement active_color castling_rights en_passant halfmove fullmove), found {}",
                parts.len()
            )));
        }

        // Extract each field into named variables
        let [piece_placement, active_color, castling_rights, en_passant, halfmove_str, fullmove_str] =
            parts.as_slice()
        else {
            unreachable!()
        };

        // Validate that no field is empty
        if piece_placement.is_empty() {
            return Err(ChessError::InvalidFen(
                "Piece placement field cannot be empty".to_string(),
            ));
        }
        if active_color.is_empty() {
            return Err(ChessError::InvalidFen(
                "Active color field cannot be empty".to_string(),
            ));
        }
        if castling_rights.is_empty() {
            return Err(ChessError::InvalidFen(
                "Castling rights field cannot be empty".to_string(),
            ));
        }
        if en_passant.is_empty() {
            return Err(ChessError::InvalidFen(
                "En passant field cannot be empty".to_string(),
            ));
        }
        if halfmove_str.is_empty() {
            return Err(ChessError::InvalidFen(
                "Halfmove clock field cannot be empty".to_string(),
            ));
        }
        if fullmove_str.is_empty() {
            return Err(ChessError::InvalidFen(
                "Fullmove number field cannot be empty".to_string(),
            ));
        }

        // Step 4.3: Parse Piece Placement (Field 1)
        let ranks: Vec<&str> = piece_placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(ChessError::InvalidFen(format!(
                "Piece placement must have exactly 8 ranks separated by '/', found {}",
                ranks.len()
            )));
        }

        // Initialize empty board
        let mut squares = [[None; 8]; 8];

        // Parse each rank (iterate from rank 8 to rank 1)
        for (rank_idx, rank_str) in ranks.iter().enumerate() {
            let board_rank = 7 - rank_idx; // FEN rank 8 = board_rank 7
            let fen_rank_number = 8 - rank_idx; // For error messages

            if rank_str.is_empty() {
                return Err(ChessError::InvalidFen(format!(
                    "Rank {} cannot be empty",
                    fen_rank_number
                )));
            }

            let mut file = 0;

            for c in rank_str.chars() {
                if file >= 8 {
                    return Err(ChessError::InvalidFen(format!(
                        "Rank {} has more than 8 squares (found character '{}' at position {})",
                        fen_rank_number,
                        c,
                        file + 1
                    )));
                }

                if c.is_ascii_digit() {
                    // Skip empty squares
                    let empty_squares = c.to_digit(10).unwrap() as usize;
                    if empty_squares == 0 || empty_squares > 8 {
                        return Err(ChessError::InvalidFen(format!(
                            "Invalid empty square count '{}' in rank {} (must be 1-8)",
                            c, fen_rank_number
                        )));
                    }
                    if file + empty_squares > 8 {
                        return Err(ChessError::InvalidFen(format!(
                            "Empty square count '{}' in rank {} would exceed 8 squares (current position: {})",
                            c, fen_rank_number, file + 1
                        )));
                    }
                    file += empty_squares;
                } else {
                    // Place piece - validate character first
                    if !c.is_ascii_alphabetic() {
                        return Err(ChessError::InvalidFen(format!(
                            "Invalid character '{}' in rank {} at position {} (expected piece letter or digit 1-8)",
                            c, fen_rank_number, file + 1
                        )));
                    }
                    let piece = Self::char_to_piece(c).map_err(|_| {
                        ChessError::InvalidFen(format!(
                            "Invalid piece character '{}' in rank {} at position {} (valid pieces: KQRBNPkqrbnp)",
                            c, fen_rank_number, file + 1
                        ))
                    })?;
                    squares[board_rank][file] = Some(piece);
                    file += 1;
                }
            }

            // Validate that we have exactly 8 squares per rank
            if file != 8 {
                return Err(ChessError::InvalidFen(format!(
                    "Rank {} must represent exactly 8 squares, found {} (check piece placement and empty square counts)",
                    fen_rank_number,
                    file
                )));
            }
        }

        // Step 4.4: Parse Active Color (Field 2)
        let parsed_active_color = match *active_color {
            "w" => Color::White,
            "b" => Color::Black,
            _ => {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid active color '{}' (must be 'w' for White or 'b' for Black)",
                    active_color
                )))
            }
        };

        // Step 4.5: Parse Castling Rights (Field 3)
        // Validate castling rights format more thoroughly
        if *castling_rights != "-" {
            // Check for invalid characters
            for c in castling_rights.chars() {
                if !"KQkq".contains(c) {
                    return Err(ChessError::InvalidFen(format!(
                        "Invalid character '{}' in castling rights '{}' (valid characters: K, Q, k, q, or '-' for none)",
                        c, castling_rights
                    )));
                }
            }

            // Check for duplicate characters
            let mut seen_chars = std::collections::HashSet::new();
            for c in castling_rights.chars() {
                if !seen_chars.insert(c) {
                    return Err(ChessError::InvalidFen(format!(
                        "Duplicate character '{}' in castling rights '{}'",
                        c, castling_rights
                    )));
                }
            }

            // Check if castling rights are in conventional order
            let chars: Vec<char> = castling_rights.chars().collect();
            let expected_order = ['K', 'Q', 'k', 'q'];
            let mut last_valid_index = -1i32;

            for &c in &chars {
                if let Some(pos) = expected_order.iter().position(|&x| x == c) {
                    if (pos as i32) < last_valid_index {
                        return Err(ChessError::InvalidFen(format!(
                            "Castling rights '{}' not in conventional order (expected order: KQkq)",
                            castling_rights
                        )));
                    }
                    last_valid_index = pos as i32;
                }
            }
        } else if castling_rights.len() != 1 {
            return Err(ChessError::InvalidFen(format!(
                "Invalid castling rights '{}' (use '-' for no castling rights)",
                castling_rights
            )));
        }
        // TODO: Store castling rights when castling is implemented

        // Step 4.6: Parse En Passant Target (Field 4)
        if *en_passant != "-" {
            // Validate it's a valid square notation (e.g., "e3", "d6")
            if en_passant.len() != 2 {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid en passant target '{}' (must be 2 characters like 'e3' or '-' for none)",
                    en_passant
                )));
            }
            let file_char = en_passant.chars().nth(0).unwrap();
            let rank_char = en_passant.chars().nth(1).unwrap();

            if !('a'..='h').contains(&file_char) {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid file '{}' in en passant target '{}' (must be a-h)",
                    file_char, en_passant
                )));
            }
            if !('1'..='8').contains(&rank_char) {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid rank '{}' in en passant target '{}' (must be 1-8)",
                    rank_char, en_passant
                )));
            }

            // Additional validation: en passant target should be on rank 3 or 6
            let rank_num = rank_char.to_digit(10).unwrap() as u8;
            if rank_num != 3 && rank_num != 6 {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid en passant target '{}' (en passant squares must be on rank 3 or 6)",
                    en_passant
                )));
            }
        } else if en_passant.len() != 1 {
            return Err(ChessError::InvalidFen(format!(
                "Invalid en passant field '{}' (use '-' for no en passant)",
                en_passant
            )));
        }
        // TODO: Store en passant target when en passant is implemented

        // Step 4.7: Parse Halfmove Clock (Field 5)
        let halfmove_clock = halfmove_str.parse::<u16>().map_err(|e| {
            ChessError::InvalidFen(format!(
                "Invalid halfmove clock '{}' (must be a non-negative integer): {}",
                halfmove_str, e
            ))
        })?;

        // Validate reasonable range for halfmove clock (0-100 is typical)
        if halfmove_clock > 100 {
            return Err(ChessError::InvalidFen(format!(
                "Halfmove clock {} is unusually high (typically 0-100, max 50 for 50-move rule)",
                halfmove_clock
            )));
        }

        // Step 4.8: Parse Fullmove Number (Field 6)
        let fullmove_number = fullmove_str.parse::<u16>().map_err(|e| {
            ChessError::InvalidFen(format!(
                "Invalid fullmove number '{}' (must be a positive integer): {}",
                fullmove_str, e
            ))
        })?;

        if fullmove_number == 0 {
            return Err(ChessError::InvalidFen(
                "Fullmove number must be at least 1".to_string(),
            ));
        }

        // Validate reasonable range for fullmove number
        if fullmove_number > 9999 {
            return Err(ChessError::InvalidFen(format!(
                "Fullmove number {} is unusually high (games rarely exceed 200 moves)",
                fullmove_number
            )));
        }

        // Step 4.9: Create and Return Board
        Ok(Board {
            squares,
            active_color: parsed_active_color,
            halfmove_clock,
            fullmove_number,
        })
    }

    /// Converts the current board state to FEN notation
    /// Returns a string in standard FEN format with 6 space-separated fields
    pub fn to_fen(&self) -> String {
        let piece_placement = self.generate_piece_placement();
        let active_color = match self.active_color {
            Color::White => "w",
            Color::Black => "b",
        };
        let castling_rights = "KQkq"; // Placeholder - will be implemented later
        let en_passant = "-"; // Placeholder - will be implemented later
        let halfmove = self.halfmove_clock;
        let fullmove = self.fullmove_number;

        format!(
            "{} {} {} {} {} {}",
            piece_placement, active_color, castling_rights, en_passant, halfmove, fullmove
        )
    }

    /// Generate the piece placement portion of FEN notation
    /// Iterates through ranks 8 down to 1, converting pieces to FEN characters
    /// and optimizing consecutive empty squares into numbers
    fn generate_piece_placement(&self) -> String {
        let mut ranks = Vec::with_capacity(8);

        // Iterate through ranks 8 down to 1 (board indices 7 down to 0)
        for rank_idx in (0..8).rev() {
            let mut rank_string = String::new();
            let mut empty_count = 0;

            // Iterate through files a-h (columns 0-7)
            for file_idx in 0..8 {
                match self.squares[rank_idx][file_idx] {
                    Some(piece) => {
                        // If we have accumulated empty squares, add the count first
                        if empty_count > 0 {
                            rank_string.push_str(&empty_count.to_string());
                            empty_count = 0;
                        }
                        // Add the piece character
                        rank_string.push(self.piece_to_fen_char(&piece));
                    }
                    None => {
                        // Count consecutive empty squares
                        empty_count += 1;
                    }
                }
            }

            // Add any remaining empty squares at the end of the rank
            if empty_count > 0 {
                rank_string.push_str(&empty_count.to_string());
            }

            ranks.push(rank_string);
        }

        ranks.join("/")
    }

    /// Convert a piece to its FEN character representation
    /// White pieces: uppercase letters (PRNBQK)
    /// Black pieces: lowercase letters (prnbqk)
    fn piece_to_fen_char(&self, piece: &Piece) -> char {
        let base_char = match piece.piece_type {
            PieceType::Pawn => 'P',
            PieceType::Rook => 'R',
            PieceType::Knight => 'N',
            PieceType::Bishop => 'B',
            PieceType::Queen => 'Q',
            PieceType::King => 'K',
        };

        match piece.color {
            Color::White => base_char,
            Color::Black => base_char.to_ascii_lowercase(),
        }
    }

    /// Optimize consecutive empty squares in a rank string by replacing them with numbers
    /// Example: "...P...." becomes "3P4"
    /// This helper method scans the rank string for consecutive dots/empty indicators
    /// and replaces sequences of empty squares with their count (1-8)
    fn optimize_empty_squares(rank_str: &str) -> String {
        let mut result = String::new();
        let mut empty_count = 0;

        for c in rank_str.chars() {
            if c == '.' {
                // Count consecutive empty squares
                empty_count += 1;
            } else {
                // We hit a piece, so first add any accumulated empty squares
                if empty_count > 0 {
                    result.push_str(&empty_count.to_string());
                    empty_count = 0;
                }
                // Add the piece character
                result.push(c);
            }
        }

        // Add any remaining empty squares at the end of the rank
        if empty_count > 0 {
            result.push_str(&empty_count.to_string());
        }

        result
    }

    /// Helper function to convert FEN piece character to Piece
    fn char_to_piece(c: char) -> Result<Piece, ChessError> {
        let (piece_type, color) = match c {
            'K' => (PieceType::King, Color::White),
            'Q' => (PieceType::Queen, Color::White),
            'R' => (PieceType::Rook, Color::White),
            'B' => (PieceType::Bishop, Color::White),
            'N' => (PieceType::Knight, Color::White),
            'P' => (PieceType::Pawn, Color::White),
            'k' => (PieceType::King, Color::Black),
            'q' => (PieceType::Queen, Color::Black),
            'r' => (PieceType::Rook, Color::Black),
            'b' => (PieceType::Bishop, Color::Black),
            'n' => (PieceType::Knight, Color::Black),
            'p' => (PieceType::Pawn, Color::Black),
            _ => {
                return Err(ChessError::InvalidFen(format!(
                    "Invalid piece character '{}'",
                    c
                )))
            }
        };
        Ok(Piece::new(piece_type, color))
    }

    /// Display the board as ASCII art from White's perspective
    /// Shows rank 8 at the top, rank 1 at the bottom
    /// Includes coordinate labels (a-h files, 1-8 ranks)
    pub fn to_ascii(&self) -> String {
        let mut result = String::new();

        // Top file labels
        result.push_str("  a b c d e f g h\n");

        // Display each rank from 8 down to 1 (White's perspective)
        for display_rank in (0..8).rev() {
            let rank_number = display_rank + 1;

            // Left rank label
            result.push_str(&format!("{} ", rank_number));

            // Display each file from a to h
            for file in 0..8 {
                let piece = self.squares[display_rank][file];
                let symbol = match piece {
                    Some(piece) => piece.to_string(),
                    None => ".".to_string(),
                };
                result.push_str(&symbol);

                // Add space between squares (except after the last square)
                if file < 7 {
                    result.push(' ');
                }
            }

            // Right rank label
            result.push_str(&format!(" {}\n", rank_number));
        }

        // Bottom file labels
        result.push_str("  a b c d e f g h");

        result
    }

    /// Apply a move to the board, updating the board state
    /// This method handles basic move application including:
    /// - Moving pieces from source to destination
    /// - Handling captures
    /// - Updating active color and move counters
    /// - Basic validation and special move detection
    pub fn make_move(&mut self, mv: Move) -> Result<(), ChessError> {
        // Basic move validation
        let source_piece = self.get_piece(mv.from).ok_or_else(|| {
            ChessError::InvalidMove(format!("No piece at source position {}", mv.from))
        })?;

        // Ensure piece belongs to active player
        if source_piece.color != self.active_color {
            return Err(ChessError::InvalidMove(format!(
                "Cannot move {} piece when it's {}'s turn",
                source_piece.color, self.active_color
            )));
        }

        // Validate destination position bounds
        if mv.to.file > 7 || mv.to.rank > 7 {
            return Err(ChessError::InvalidMove(format!(
                "Destination position {} is out of bounds",
                mv.to
            )));
        }

        // Check for friendly fire (can't capture own pieces)
        if let Some(dest_piece) = self.get_piece(mv.to) {
            if dest_piece.color == self.active_color {
                return Err(ChessError::InvalidMove(format!(
                    "Cannot capture own piece at {}",
                    mv.to
                )));
            }
        }

        // Handle special moves detection and validation
        let is_capture = self.get_piece(mv.to).is_some();
        let is_pawn_move = source_piece.piece_type == PieceType::Pawn;
        let is_castling = self.detect_castling_move(&mv, &source_piece)?;
        let is_en_passant = self.detect_en_passant_move(&mv, &source_piece)?;

        // Handle pawn promotion validation
        if let Some(_promotion_piece) = mv.promotion {
            if !is_pawn_move {
                return Err(ChessError::InvalidMove(
                    "Only pawns can be promoted".to_string(),
                ));
            }

            // Check if pawn is reaching the promotion rank
            let promotion_rank = match self.active_color {
                Color::White => 7, // rank 8
                Color::Black => 0, // rank 1
            };

            if mv.to.rank != promotion_rank {
                return Err(ChessError::InvalidMove(format!(
                    "Pawn promotion only allowed when reaching rank {}",
                    promotion_rank + 1
                )));
            }
        } else if is_pawn_move {
            // Check if promotion is required but not provided
            let promotion_rank = match self.active_color {
                Color::White => 7, // rank 8
                Color::Black => 0, // rank 1
            };

            if mv.to.rank == promotion_rank {
                return Err(ChessError::InvalidMove(
                    "Pawn promotion required when reaching the last rank".to_string(),
                ));
            }
        }

        // Apply the move
        if is_castling {
            self.apply_castling_move(&mv)?;
        } else if is_en_passant {
            self.apply_en_passant_move(&mv)?;
        } else {
            // Standard move application
            self.apply_standard_move(&mv)?;
        }

        // Update move counters
        self.update_move_counters(is_pawn_move, is_capture);

        // Switch active color
        self.active_color = match self.active_color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };

        Ok(())
    }

    /// Placeholder for legal move validation
    /// Returns true for all moves (to be implemented in future phases)
    pub fn is_legal_move(&self, _mv: Move) -> bool {
        // TODO: Implement comprehensive legal move validation including:
        // - Piece-specific movement rules
        // - Check detection and prevention
        // - Castling legality (king/rook not moved, no pieces between, not in check)
        // - En passant legality (pawn just moved two squares)
        // - Pin detection and handling
        true
    }

    /// Detect if a move is a castling move
    fn detect_castling_move(&self, mv: &Move, piece: &Piece) -> Result<bool, ChessError> {
        if piece.piece_type != PieceType::King {
            return Ok(false);
        }

        // Check if king moves exactly 2 squares horizontally
        if mv.from.rank == mv.to.rank && mv.from.file.abs_diff(mv.to.file) == 2 {
            // Validate castling move is on the correct rank
            let expected_rank = match self.active_color {
                Color::White => 0, // rank 1
                Color::Black => 7, // rank 8
            };

            if mv.from.rank != expected_rank {
                return Err(ChessError::InvalidMove(format!(
                    "Castling must be performed on rank {} for {}",
                    expected_rank + 1,
                    self.active_color
                )));
            }

            // Validate king starts from e-file
            if mv.from.file != 4 {
                return Err(ChessError::InvalidMove(
                    "Castling king must start from e-file".to_string(),
                ));
            }

            return Ok(true);
        }

        Ok(false)
    }

    /// Detect if a move is an en passant move
    fn detect_en_passant_move(&self, mv: &Move, piece: &Piece) -> Result<bool, ChessError> {
        if piece.piece_type != PieceType::Pawn {
            return Ok(false);
        }

        // Check if pawn moves diagonally to an empty square
        if mv.from.file != mv.to.file && mv.from.rank.abs_diff(mv.to.rank) == 1 {
            if self.get_piece(mv.to).is_none() {
                // This could be en passant - validate the move direction
                let expected_direction = match self.active_color {
                    Color::White => 1,  // white pawns move up (increasing rank)
                    Color::Black => -1, // black pawns move down (decreasing rank)
                };

                let actual_direction = (mv.to.rank as i8) - (mv.from.rank as i8);
                if actual_direction != expected_direction {
                    return Err(ChessError::InvalidMove(
                        "Pawn moving in wrong direction".to_string(),
                    ));
                }

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Apply a standard (non-special) move
    fn apply_standard_move(&mut self, mv: &Move) -> Result<(), ChessError> {
        let piece = self.get_piece(mv.from).unwrap(); // Already validated in make_move

        // Handle promotion
        let final_piece = if let Some(promotion_type) = mv.promotion {
            Piece::new(promotion_type, piece.color)
        } else {
            piece
        };

        // Remove piece from source
        self.set_piece(mv.from, None)?;

        // Place piece at destination (handles captures automatically)
        self.set_piece(mv.to, Some(final_piece))?;

        Ok(())
    }

    /// Apply a castling move (king and rook movement)
    fn apply_castling_move(&mut self, mv: &Move) -> Result<(), ChessError> {
        // Move the king
        let king = self.get_piece(mv.from).unwrap(); // Already validated
        self.set_piece(mv.from, None)?;
        self.set_piece(mv.to, Some(king))?;

        // Determine rook positions and move the rook
        let (rook_from_file, rook_to_file) = if mv.to.file == 6 {
            // Kingside castling (O-O): rook moves from h-file to f-file
            (7, 5)
        } else if mv.to.file == 2 {
            // Queenside castling (O-O-O): rook moves from a-file to d-file
            (0, 3)
        } else {
            return Err(ChessError::InvalidMove(
                "Invalid castling destination".to_string(),
            ));
        };

        let rook_from = Position::new_unchecked(rook_from_file, mv.from.rank);
        let rook_to = Position::new_unchecked(rook_to_file, mv.from.rank);

        // Validate rook exists
        let rook = self.get_piece(rook_from).ok_or_else(|| {
            ChessError::InvalidMove(format!("No rook found at {} for castling", rook_from))
        })?;

        if rook.piece_type != PieceType::Rook || rook.color != self.active_color {
            return Err(ChessError::InvalidMove(
                "Invalid rook for castling".to_string(),
            ));
        }

        // Move the rook
        self.set_piece(rook_from, None)?;
        self.set_piece(rook_to, Some(rook))?;

        Ok(())
    }

    /// Apply an en passant move (pawn capture and removal of captured pawn)
    fn apply_en_passant_move(&mut self, mv: &Move) -> Result<(), ChessError> {
        let pawn = self.get_piece(mv.from).unwrap(); // Already validated

        // Move the pawn to the destination
        self.set_piece(mv.from, None)?;
        self.set_piece(mv.to, Some(pawn))?;

        // Remove the captured pawn (which is on the same rank as the source)
        let captured_pawn_pos = Position::new_unchecked(mv.to.file, mv.from.rank);
        let captured_pawn = self.get_piece(captured_pawn_pos).ok_or_else(|| {
            ChessError::InvalidMove("No pawn to capture for en passant".to_string())
        })?;

        // Validate the captured piece is an enemy pawn
        if captured_pawn.piece_type != PieceType::Pawn || captured_pawn.color == self.active_color {
            return Err(ChessError::InvalidMove(
                "Invalid piece for en passant capture".to_string(),
            ));
        }

        self.set_piece(captured_pawn_pos, None)?;

        Ok(())
    }

    /// Update move counters based on the move type
    fn update_move_counters(&mut self, is_pawn_move: bool, is_capture: bool) {
        // Update halfmove clock (50-move rule)
        if is_pawn_move || is_capture {
            // Reset halfmove clock on pawn moves and captures
            self.halfmove_clock = 0;
        } else {
            // Increment halfmove clock
            self.halfmove_clock += 1;
        }

        // Update fullmove number (increments after Black's move)
        if self.active_color == Color::Black {
            self.fullmove_number += 1;
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}
