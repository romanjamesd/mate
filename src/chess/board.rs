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
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}
