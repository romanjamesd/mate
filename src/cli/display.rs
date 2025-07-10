use crate::chess::{Board, Color};
use crate::cli::GameRecord;
use crate::storage::models::GameStatus;
use std::io::{self, Write};

/// Display a list of games in a pretty ASCII table format
pub fn display_games_list(games: &[GameRecord]) {
    if games.is_empty() {
        println!("No games found.");
        return;
    }

    // Calculate column widths dynamically
    let id_width = games
        .iter()
        .map(|g| g.game.id.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let opponent_width = games
        .iter()
        .map(|g| g.opponent_name.as_ref().map_or(8, |n| n.len()))
        .max()
        .unwrap_or(8)
        .max(8);
    let status_width = 9; // "completed" is the longest status
    let color_width = 5; // "Black" is the longest color
    let moves_width = 5; // "Moves" header width
    let turn_width = 4; // "Turn" header width

    // Print header
    println!(
        "┌{:─<width_id$}┬{:─<width_opp$}┬{:─<width_stat$}┬{:─<width_col$}┬{:─<width_mov$}┬{:─<width_turn$}┐",
        "",
        "",
        "",
        "",
        "",
        "",
        width_id = id_width + 2,
        width_opp = opponent_width + 2,
        width_stat = status_width + 2,
        width_col = color_width + 2,
        width_mov = moves_width + 2,
        width_turn = turn_width + 2
    );

    println!(
        "│ {:^width_id$} │ {:^width_opp$} │ {:^width_stat$} │ {:^width_col$} │ {:^width_mov$} │ {:^width_turn$} │",
        "Game ID",
        "Opponent",
        "Status",
        "Color",
        "Moves",
        "Turn",
        width_id = id_width,
        width_opp = opponent_width,
        width_stat = status_width,
        width_col = color_width,
        width_mov = moves_width,
        width_turn = turn_width
    );

    println!(
        "├{:─<width_id$}┼{:─<width_opp$}┼{:─<width_stat$}┼{:─<width_col$}┼{:─<width_mov$}┼{:─<width_turn$}┤",
        "",
        "",
        "",
        "",
        "",
        "",
        width_id = id_width + 2,
        width_opp = opponent_width + 2,
        width_stat = status_width + 2,
        width_col = color_width + 2,
        width_mov = moves_width + 2,
        width_turn = turn_width + 2
    );

    // Print each game row
    for game in games {
        let game_id = &game.game.id[..id_width.min(game.game.id.len())];
        let opponent = game
            .opponent_name
            .as_ref()
            .map(|n| {
                if n.len() > opponent_width {
                    {
                        let truncated = &n[..opponent_width - 3];
                        format!("{truncated}...")
                    }
                } else {
                    n.clone()
                }
            })
            .unwrap_or_else(|| {
                let truncated =
                    &game.game.opponent_peer_id[..8.min(game.game.opponent_peer_id.len())];
                format!("{truncated}...")
            });

        let status = match game.game.status {
            GameStatus::Pending => "⏳ Pending",
            GameStatus::Active => "🎮 Active",
            GameStatus::Completed => "✅ Done",
            GameStatus::Abandoned => "❌ Abandoned",
        };

        let color = match game.game.my_color {
            crate::storage::models::PlayerColor::White => "⚪ White",
            crate::storage::models::PlayerColor::Black => "⚫ Black",
        };

        let turn_indicator = if game.your_turn {
            "👤 You"
        } else {
            "👥 Them"
        };

        println!(
            "│ {:width_id$} │ {:width_opp$} │ {:width_stat$} │ {:width_col$} │ {:>width_mov$} │ {:width_turn$} │",
            game_id,
            opponent,
            status,
            color,
            game.move_count,
            turn_indicator,
            width_id = id_width,
            width_opp = opponent_width,
            width_stat = status_width,
            width_col = color_width,
            width_mov = moves_width,
            width_turn = turn_width
        );
    }

    // Print footer
    println!(
        "└{:─<width_id$}┴{:─<width_opp$}┴{:─<width_stat$}┴{:─<width_col$}┴{:─<width_mov$}┴{:─<width_turn$}┘",
        "",
        "",
        "",
        "",
        "",
        "",
        width_id = id_width + 2,
        width_opp = opponent_width + 2,
        width_stat = status_width + 2,
        width_col = color_width + 2,
        width_mov = moves_width + 2,
        width_turn = turn_width + 2
    );

    println!("\n{} game(s) total", games.len());
}

/// Display a chess board from the specified perspective
/// If perspective is White, displays from White's perspective (rank 1 at bottom)
/// If perspective is Black, displays from Black's perspective (rank 8 at bottom)
pub fn display_board(board: &Board, perspective: Color) {
    println!();

    match perspective {
        Color::White => display_board_white_perspective(board),
        Color::Black => display_board_black_perspective(board),
    }

    // Show game status information
    println!("To move: {}", board.active_color());
    println!("Move #: {}", board.fullmove_number());

    if board.halfmove_clock() > 0 {
        println!("Halfmove clock: {} (50-move rule)", board.halfmove_clock());
    }
}

/// Display board with Unicode pieces (default)
pub fn display_board_unicode(board: &Board, perspective: Color) {
    display_board(board, perspective);
}

/// Display board with ASCII pieces (fallback for terminals without Unicode support)
pub fn display_board_ascii(board: &Board, perspective: Color) {
    println!();

    match perspective {
        Color::White => display_board_white_perspective_ascii(board),
        Color::Black => display_board_black_perspective_ascii(board),
    }

    // Show game status information
    println!("To move: {}", board.active_color());
    println!("Move #: {}", board.fullmove_number());

    if board.halfmove_clock() > 0 {
        println!("Halfmove clock: {} (50-move rule)", board.halfmove_clock());
    }
}

/// Display board from White's perspective (rank 1 at bottom)
fn display_board_white_perspective(board: &Board) {
    // Top border
    println!("  ┌─┬─┬─┬─┬─┬─┬─┬─┐");

    // Display each rank from 8 down to 1 (White's perspective)
    for display_rank in (0..8).rev() {
        let rank_number = display_rank + 1;

        // Print rank with pieces
        print!("{} │", rank_number);
        for file in 0..8 {
            let piece = board.get_piece(crate::chess::Position {
                file: file as u8,
                rank: display_rank as u8,
            });

            let symbol = match piece {
                Some(piece) => piece.to_string(),
                None => " ".to_string(),
            };
            print!("{}│", symbol);
        }
        println!(" {}", rank_number);

        // Print separator (except after last rank)
        if display_rank > 0 {
            println!("  ├─┼─┼─┼─┼─┼─┼─┼─┤");
        }
    }

    // Bottom border
    println!("  └─┴─┴─┴─┴─┴─┴─┴─┘");
    println!("   a b c d e f g h");
}

/// Display board from Black's perspective (rank 8 at bottom)
fn display_board_black_perspective(board: &Board) {
    // Top border
    println!("  ┌─┬─┬─┬─┬─┬─┬─┬─┐");

    // Display each rank from 1 up to 8 (Black's perspective)
    for display_rank in 0..8 {
        let rank_number = display_rank + 1;

        // Print rank with pieces (files reversed for Black's perspective)
        print!("{} │", rank_number);
        for file in (0..8).rev() {
            let piece = board.get_piece(crate::chess::Position {
                file: file as u8,
                rank: display_rank as u8,
            });

            let symbol = match piece {
                Some(piece) => piece.to_string(),
                None => " ".to_string(),
            };
            print!("{}│", symbol);
        }
        println!(" {}", rank_number);

        // Print separator (except after last rank)
        if display_rank < 7 {
            println!("  ├─┼─┼─┼─┼─┼─┼─┼─┤");
        }
    }

    // Bottom border
    println!("  └─┴─┴─┴─┴─┴─┴─┴─┘");
    println!("   h g f e d c b a");
}

/// Display board from White's perspective with ASCII pieces
fn display_board_white_perspective_ascii(board: &Board) {
    // Top border
    println!("  ┌─┬─┬─┬─┬─┬─┬─┬─┐");

    // Display each rank from 8 down to 1 (White's perspective)
    for display_rank in (0..8).rev() {
        let rank_number = display_rank + 1;

        print!("{} │", rank_number);
        for file in 0..8 {
            let piece = board.get_piece(crate::chess::Position {
                file: file as u8,
                rank: display_rank as u8,
            });

            let symbol = match piece {
                Some(piece) => piece_to_ascii_char(&piece),
                None => " ".to_string(),
            };
            print!("{}│", symbol);
        }
        println!(" {}", rank_number);

        // Print separator (except after last rank)
        if display_rank > 0 {
            println!("  ├─┼─┼─┼─┼─┼─┼─┼─┤");
        }
    }

    // Bottom border
    println!("  └─┴─┴─┴─┴─┴─┴─┴─┘");
    println!("   a b c d e f g h");
}

/// Display board from Black's perspective with ASCII pieces
fn display_board_black_perspective_ascii(board: &Board) {
    // Top border
    println!("  ┌─┬─┬─┬─┬─┬─┬─┬─┐");

    // Display each rank from 1 up to 8 (Black's perspective)
    for display_rank in 0..8 {
        let rank_number = display_rank + 1;

        print!("{} │", rank_number);
        for file in (0..8).rev() {
            let piece = board.get_piece(crate::chess::Position {
                file: file as u8,
                rank: display_rank as u8,
            });

            let symbol = match piece {
                Some(piece) => piece_to_ascii_char(&piece),
                None => " ".to_string(),
            };
            print!("{}│", symbol);
        }
        println!(" {}", rank_number);

        // Print separator (except after last rank)
        if display_rank < 7 {
            println!("  ├─┼─┼─┼─┼─┼─┼─┼─┤");
        }
    }

    // Bottom border
    println!("  └─┴─┴─┴─┴─┴─┴─┴─┘");
    println!("   h g f e d c b a");
}

/// Convert a piece to ASCII character representation
fn piece_to_ascii_char(piece: &crate::chess::Piece) -> String {
    use crate::chess::{Color, PieceType};

    let symbol = match (piece.color, piece.piece_type) {
        (Color::White, PieceType::Pawn) => "P",
        (Color::White, PieceType::Rook) => "R",
        (Color::White, PieceType::Knight) => "N",
        (Color::White, PieceType::Bishop) => "B",
        (Color::White, PieceType::Queen) => "Q",
        (Color::White, PieceType::King) => "K",
        (Color::Black, PieceType::Pawn) => "p",
        (Color::Black, PieceType::Rook) => "r",
        (Color::Black, PieceType::Knight) => "n",
        (Color::Black, PieceType::Bishop) => "b",
        (Color::Black, PieceType::Queen) => "q",
        (Color::Black, PieceType::King) => "k",
    };
    symbol.to_string()
}

/// Display move history in a formatted table
pub fn display_move_history(history: &[String], current_move: u32) {
    if history.is_empty() {
        println!("No moves in history.");
        return;
    }

    println!("\nMove History");
    println!("┌──────┬─────────┬─────────┐");
    println!("│ Move │  White  │  Black  │");
    println!("├──────┼─────────┼─────────┤");

    for (i, move_pair) in history.chunks(2).enumerate() {
        let move_num = i + 1;
        let white_move = move_pair.first().map(|s| s.as_str()).unwrap_or("-");
        let black_move = move_pair.get(1).map(|s| s.as_str()).unwrap_or("-");

        println!(
            "│ {:>4} │ {:^7} │ {:^7} │",
            move_num, white_move, black_move
        );
    }

    println!("└──────┴─────────┴─────────┘");
    println!("Current move: {}", current_move);
}

/// Display game status with color coding
pub fn display_game_status(
    status: &GameStatus,
    result: Option<&crate::storage::models::GameResult>,
) {
    match status {
        GameStatus::Pending => println!("⏳ Game Status: Waiting for opponent to accept"),
        GameStatus::Active => println!("🎮 Game Status: In progress"),
        GameStatus::Completed => match result {
            Some(crate::storage::models::GameResult::Win) => println!("🏆 Game Status: You won!"),
            Some(crate::storage::models::GameResult::Loss) => println!("😞 Game Status: You lost"),
            Some(crate::storage::models::GameResult::Draw) => println!("🤝 Game Status: Draw"),
            Some(crate::storage::models::GameResult::Abandoned) => {
                println!("🚫 Game Status: Abandoned")
            }
            None => println!("✅ Game Status: Completed"),
        },
        GameStatus::Abandoned => println!("❌ Game Status: Abandoned"),
    }
}

/// Check if terminal supports Unicode chess pieces
pub fn supports_unicode() -> bool {
    // Simple heuristic: check if TERM contains "xterm" or if we're in a modern terminal
    std::env::var("TERM")
        .map(|term| {
            term.contains("xterm")
                || term.contains("screen")
                || term.contains("tmux")
                || term == "alacritty"
                || term == "kitty"
        })
        .unwrap_or(false)
        || std::env::var("TERM_PROGRAM").is_ok() // macOS Terminal, iTerm2, etc.
}

/// Interactive function to get user's display preference
pub fn get_display_preference() -> bool {
    if supports_unicode() {
        print!("Use Unicode chess pieces? [Y/n]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim().to_lowercase().as_str() {
            "n" | "no" | "ascii" => false,
            _ => true, // Default to Unicode
        }
    } else {
        false // Fall back to ASCII for terminals that don't support Unicode
    }
}
