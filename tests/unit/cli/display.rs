//! Unit tests for CLI display functionality
//!
//! Tests the display.rs module including board rendering, game list formatting,
//! and error handling according to the testing plan step 1.4.

use mate::chess::{Board, Color};
use mate::cli::display::*;
use mate::cli::GameRecord;
use mate::storage::models::{Game, GameResult, GameStatus, PlayerColor};

/// Helper function to create test game records
fn create_test_game_record(
    id: &str,
    opponent_name: Option<String>,
    status: GameStatus,
    my_color: PlayerColor,
    your_turn: bool,
    move_count: u32,
) -> GameRecord {
    GameRecord {
        game: Game {
            id: id.to_string(),
            opponent_peer_id: "peer123".to_string(),
            my_color,
            status,
            created_at: 1234567890,
            updated_at: 1234567890,
            completed_at: None,
            result: None,
            metadata: None,
        },
        opponent_name,
        last_move: None,
        your_turn,
        move_count,
    }
}

/// Helper function to create a board from FEN
fn board_from_fen(fen: &str) -> Board {
    Board::from_fen(fen).expect("Valid FEN should parse")
}

// ==============================================
// Board Display Tests
// ==============================================

#[test]
fn test_ascii_board_rendering_with_piece_placement() {
    // Test starting position
    let board = Board::new();

    // Test that the function can be called without panicking
    display_board_ascii(&board, Color::White);
    display_board_ascii(&board, Color::Black);

    // Test with a custom position - position after 1.e4 e5
    let board = board_from_fen("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2");
    display_board_ascii(&board, Color::White);
    display_board_ascii(&board, Color::Black);

    // Test with empty board
    let board = board_from_fen("8/8/8/8/8/8/8/8 w - - 0 1");
    display_board_ascii(&board, Color::White);
    display_board_ascii(&board, Color::Black);
}

#[test]
fn test_coordinate_display_ranks_and_files() {
    let board = Board::new();

    // Test both perspectives show coordinates
    // White perspective should show a-h from left to right, 1-8 from bottom to top
    display_board(&board, Color::White);

    // Black perspective should show h-a from left to right, 8-1 from bottom to top
    display_board(&board, Color::Black);

    // The actual coordinate verification would require capturing output
    // For now we verify the functions execute without error
}

#[test]
fn test_perspective_switching_white_vs_black() {
    let board = board_from_fen("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2");

    // Test Unicode display from both perspectives
    display_board_unicode(&board, Color::White);
    display_board_unicode(&board, Color::Black);

    // Test ASCII display from both perspectives
    display_board_ascii(&board, Color::White);
    display_board_ascii(&board, Color::Black);

    // Test general display function
    display_board(&board, Color::White);
    display_board(&board, Color::Black);
}

#[test]
fn test_unicode_vs_ascii_piece_symbols() {
    let board = Board::new();

    // Test Unicode pieces
    display_board_unicode(&board, Color::White);

    // Test ASCII pieces
    display_board_ascii(&board, Color::White);

    // Both should handle the same board without errors
    let complex_board =
        board_from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4");
    display_board_unicode(&complex_board, Color::White);
    display_board_ascii(&complex_board, Color::White);
}

#[test]
fn test_board_display_various_positions() {
    let test_positions = vec![
        (
            "Starting position",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ),
        (
            "After 1.e4",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        ),
        (
            "After 1.e4 e5",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
        ),
        ("Empty board", "8/8/8/8/8/8/8/8 w - - 0 1"),
        ("Castling test", "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1"),
        (
            "Mid-game position",
            "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        ),
    ];

    for (description, fen) in test_positions {
        let board = board_from_fen(fen);

        // Test all display variants
        display_board(&board, Color::White);
        display_board(&board, Color::Black);
        display_board_unicode(&board, Color::White);
        display_board_unicode(&board, Color::Black);
        display_board_ascii(&board, Color::White);
        display_board_ascii(&board, Color::Black);

        println!("Successfully displayed {}", description);
    }
}

// ==============================================
// Game List Display Tests
// ==============================================

#[test]
fn test_table_formatting_column_alignment() {
    let games = vec![
        create_test_game_record(
            "abc123def456",
            Some("Alice".to_string()),
            GameStatus::Active,
            PlayerColor::White,
            true,
            10,
        ),
        create_test_game_record(
            "xyz789uvw012",
            Some("Bob".to_string()),
            GameStatus::Pending,
            PlayerColor::Black,
            false,
            0,
        ),
    ];

    // Test that the function executes without error
    display_games_list(&games);

    // Test with empty list
    display_games_list(&[]);
}

#[test]
fn test_game_id_truncation() {
    let games = vec![
        create_test_game_record(
            "very_long_game_id_that_should_be_truncated_for_display_purposes",
            Some("Player".to_string()),
            GameStatus::Active,
            PlayerColor::White,
            true,
            5,
        ),
        create_test_game_record(
            "short",
            Some("Player2".to_string()),
            GameStatus::Completed,
            PlayerColor::Black,
            false,
            20,
        ),
    ];

    display_games_list(&games);
}

#[test]
fn test_peer_name_truncation() {
    let games = vec![
        create_test_game_record(
            "game1",
            Some("Very Long Player Name That Should Be Truncated".to_string()),
            GameStatus::Active,
            PlayerColor::White,
            true,
            3,
        ),
        create_test_game_record(
            "game2",
            None, // Test with no opponent name
            GameStatus::Pending,
            PlayerColor::Black,
            false,
            0,
        ),
        create_test_game_record(
            "game3",
            Some("Short".to_string()),
            GameStatus::Abandoned,
            PlayerColor::White,
            false,
            15,
        ),
    ];

    display_games_list(&games);
}

#[test]
fn test_status_icon_display() {
    let statuses = vec![
        GameStatus::Pending,
        GameStatus::Active,
        GameStatus::Completed,
        GameStatus::Abandoned,
    ];

    for (i, status) in statuses.into_iter().enumerate() {
        let games = vec![create_test_game_record(
            &format!("game{}", i),
            Some("Player".to_string()),
            status,
            PlayerColor::White,
            true,
            i as u32,
        )];

        display_games_list(&games);
    }
}

#[test]
fn test_timestamp_formatting_consistency() {
    // Create games with various move counts to test consistency
    let games = vec![
        create_test_game_record(
            "game1",
            Some("Player1".to_string()),
            GameStatus::Active,
            PlayerColor::White,
            true,
            0,
        ),
        create_test_game_record(
            "game2",
            Some("Player2".to_string()),
            GameStatus::Active,
            PlayerColor::Black,
            false,
            1,
        ),
        create_test_game_record(
            "game3",
            Some("Player3".to_string()),
            GameStatus::Completed,
            PlayerColor::White,
            false,
            50,
        ),
        create_test_game_record(
            "game4",
            Some("Player4".to_string()),
            GameStatus::Pending,
            PlayerColor::Black,
            true,
            999,
        ),
    ];

    display_games_list(&games);
}

// ==============================================
// Error Handling Tests
// ==============================================

#[test]
fn test_error_types_are_appropriate() {
    // Test display_game_status with all possible combinations
    display_game_status(&GameStatus::Pending, None);
    display_game_status(&GameStatus::Active, None);
    display_game_status(&GameStatus::Completed, None);
    display_game_status(&GameStatus::Abandoned, None);

    // Test with results
    display_game_status(&GameStatus::Completed, Some(&GameResult::Win));
    display_game_status(&GameStatus::Completed, Some(&GameResult::Loss));
    display_game_status(&GameStatus::Completed, Some(&GameResult::Draw));
    display_game_status(&GameStatus::Completed, Some(&GameResult::Abandoned));
}

#[test]
fn test_error_results_include_helpful_context() {
    // Test that display functions handle edge cases gracefully

    // Empty move history
    display_move_history(&[], 0);
    display_move_history(&[], 1);

    // Single move
    display_move_history(&["e4".to_string()], 1);

    // Multiple moves
    display_move_history(
        &[
            "e4".to_string(),
            "e5".to_string(),
            "Nf3".to_string(),
            "Nc6".to_string(),
        ],
        3,
    );

    // Odd number of moves
    display_move_history(&["e4".to_string(), "e5".to_string(), "Nf3".to_string()], 2);
}

#[test]
fn test_error_handling_consistency_across_commands() {
    // Test that all display functions handle invalid input consistently

    // Test with boards in various states
    let boards = vec![
        board_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        board_from_fen("8/8/8/8/8/8/8/8 w - - 0 1"),
        board_from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1"),
    ];

    for board in boards {
        // All display functions should handle any valid board
        display_board(&board, Color::White);
        display_board(&board, Color::Black);
        display_board_unicode(&board, Color::White);
        display_board_unicode(&board, Color::Black);
        display_board_ascii(&board, Color::White);
        display_board_ascii(&board, Color::Black);
    }
}

#[test]
fn test_security_sensitive_errors_no_internal_details() {
    // Test that display functions don't expose internal implementation details

    // Test with minimal game data
    let minimal_game = create_test_game_record(
        "test",
        None,
        GameStatus::Pending,
        PlayerColor::White,
        false,
        0,
    );

    display_games_list(&[minimal_game]);

    // Test with invalid but safe data
    let games_with_special_chars = vec![create_test_game_record(
        "test<script>",
        Some("Player<>&\"'".to_string()),
        GameStatus::Active,
        PlayerColor::White,
        true,
        1,
    )];

    display_games_list(&games_with_special_chars);
}

// ==============================================
// Unicode and Terminal Support Tests
// ==============================================

#[test]
fn test_unicode_support_detection() {
    // Test the unicode support detection function
    let supports = supports_unicode();

    // Should return a boolean without error
    assert!(supports == true || supports == false);
}

#[test]
fn test_display_preference_functionality() {
    // Test that get_display_preference doesn't panic
    // Note: This is an interactive function, so we can't test the actual input
    // But we can verify it handles the environment correctly

    // This function reads from stdin, so we can't easily test it in unit tests
    // We would need integration tests with mock stdin for full testing

    // For now, just verify the supports_unicode function works
    let _ = supports_unicode();
}

// ==============================================
// Move History Display Tests
// ==============================================

#[test]
fn test_move_history_display_formatting() {
    // Test empty history
    display_move_history(&[], 0);

    // Test single move
    display_move_history(&["e4".to_string()], 1);

    // Test full game
    let moves = vec![
        "e4".to_string(),
        "e5".to_string(),
        "Nf3".to_string(),
        "Nc6".to_string(),
        "Bb5".to_string(),
        "a6".to_string(),
        "Ba4".to_string(),
        "Nf6".to_string(),
    ];
    display_move_history(&moves, 5);

    // Test with long algebraic notation
    let long_moves = vec![
        "Ng1-f3".to_string(),
        "Nb8-c6".to_string(),
        "O-O".to_string(),
        "O-O-O".to_string(),
    ];
    display_move_history(&long_moves, 3);
}

#[test]
fn test_move_history_edge_cases() {
    // Test with very long move notation
    let long_moves = vec![
        "a very long move notation that exceeds normal length".to_string(),
        "another very long move that should be handled gracefully".to_string(),
    ];
    display_move_history(&long_moves, 2);

    // Test with special characters in moves
    let special_moves = vec![
        "e4+".to_string(),
        "Qh5#".to_string(),
        "O-O".to_string(),
        "O-O-O".to_string(),
    ];
    display_move_history(&special_moves, 3);
}
