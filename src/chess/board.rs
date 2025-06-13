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
        let mut fen_parts = Vec::with_capacity(6);

        // 1. Piece placement
        fen_parts.push(self.generate_piece_placement());

        // 2. Active color
        fen_parts.push(
            match self.active_color {
                Color::White => "w",
                Color::Black => "b",
            }
            .to_string(),
        );

        // 3. Castling rights (placeholder)
        fen_parts.push("KQkq".to_string()); // TODO: Implement proper castling tracking

        // 4. En passant target (placeholder)
        fen_parts.push("-".to_string()); // TODO: Implement en passant tracking

        // 5. Halfmove clock
        fen_parts.push(self.halfmove_clock.to_string());

        // 6. Fullmove number
        fen_parts.push(self.fullmove_number.to_string());

        fen_parts.join(" ")
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
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}


