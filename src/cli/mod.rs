pub mod app;
pub mod commands;
pub mod display;
pub mod error_handler;
pub mod network_manager;
pub mod validation;

pub use app::{App, Config};
pub use commands::{Cli, Commands, KeyCommand};
pub use display::{
    display_board, display_board_ascii, display_board_unicode, display_game_status,
    display_games_list, display_move_history, get_display_preference, supports_unicode,
};
pub use error_handler::{
    cli_error_from_anyhow, create_input_validation_error, create_network_timeout_error,
    display_error, display_error_and_exit, handle_chess_command_error, is_recoverable_error,
    CliError, CliResult,
};
pub use network_manager::{
    GameAcceptOutcome, NetworkConfig, NetworkManager, NetworkStats, SendOutcome,
};
pub use validation::{InputValidationUtils, InputValidator, ValidationError, ValidationResult};
