use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use mate::cli::{app::App, Cli, Commands, KeyCommand};
use mate::crypto::Identity;
use mate::messages::Message;
use mate::network::Client;

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::signal;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

/// Format round-trip time for display with appropriate precision
fn format_round_trip_time(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    let micros = duration.as_micros();

    if millis == 0 {
        format!("{micros}μs")
    } else if millis < 1000 {
        format!("{millis}ms")
    } else {
        let seconds = duration.as_secs_f64();
        format!("{seconds:.2}s")
    }
}

/// Initialize identity using secure storage
pub async fn init_identity() -> Result<Identity> {
    Identity::load_or_generate()
}

/// Set up graceful shutdown signal handling
async fn setup_shutdown_signal() -> Result<()> {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("Received Ctrl+C signal, initiating graceful shutdown...");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
        info!("Received SIGTERM signal, initiating graceful shutdown...");
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    Ok(())
}

/// Gracefully shutdown the application with proper cleanup
async fn graceful_shutdown(app: Option<&App>) -> Result<()> {
    info!("Starting graceful shutdown sequence...");

    if let Some(_app) = app {
        debug!("Cleaning up application resources...");

        // The database connections will be automatically closed when the Database
        // struct is dropped, but we can log the cleanup for transparency
        debug!("Database connections will be closed automatically");
        debug!("Application state cleanup complete");
    }

    info!("Graceful shutdown completed successfully");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Step 5.1: Initialize tracing with appropriate logging levels for network operations
    // Set up structured logging with appropriate levels for production use
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive("mate=info".parse()?)
                .with_env_var("RUST_LOG")
                .from_env_lossy(),
        )
        .with_target(false) // Hide target module in logs for cleaner output
        .with_level(true) // Show log levels
        .with_file(false) // Hide file names for production
        .with_line_number(false) // Hide line numbers for production
        .init();

    info!("Starting mate application with network-optimized logging configuration");
    debug!("Application lifecycle: Main function started");
    debug!("Application lifecycle: Parsing command line arguments");

    let cli = Cli::parse();
    debug!("Application lifecycle: Command line arguments parsed successfully");

    match cli.command {
        Commands::Init => {
            warn!("The 'init' command is deprecated. Use 'mate key generate' instead.");
            info!("Initializing new identity...");

            // Get the key path once and reuse it
            let key_path = match mate::crypto::storage::default_key_path() {
                Ok(path) => path,
                Err(e) => {
                    error!("Failed to determine key storage path: {}", e);
                    return Ok(());
                }
            };

            // Check if identity already exists
            if key_path.exists() {
                error!("Identity already exists at {}", key_path.display());
                return Ok(());
            }

            let identity = Identity::generate()?;
            identity.save_to_default_storage()?;

            info!("Identity created successfully!");
            info!("Peer ID: {}", identity.peer_id());
            info!("Saved to: {}", key_path.display());
        }
        Commands::Info => {
            warn!("The 'info' command is deprecated. Use 'mate key info' instead.");
            info!("Showing identity information...");

            match Identity::from_default_storage() {
                Ok(identity) => {
                    info!("Peer ID: {}", identity.peer_id());
                    info!(
                        "Public Key: {}",
                        general_purpose::STANDARD.encode(identity.verifying_key().to_bytes())
                    );
                    if let Ok(path) = mate::crypto::storage::default_key_path() {
                        info!("Storage location: {}", path.display());
                    }
                }
                Err(e) => {
                    error!("No identity found: {}", e);
                    info!("Run 'mate init' or 'mate key generate' to create a new identity");
                }
            }
        }
        Commands::Key { command } => {
            match command {
                KeyCommand::Path => {
                    info!("Showing default key storage path...");
                    match mate::crypto::storage::default_key_path() {
                        Ok(path) => {
                            info!("Default key storage path: {}", path.display());
                            if path.exists() {
                                info!("✓ Identity file exists");
                            } else {
                                info!("✗ Identity file does not exist");
                                info!("Run 'mate key generate' to create a new identity");
                            }
                        }
                        Err(e) => {
                            error!("Failed to determine key storage path: {}", e);
                        }
                    }
                }
                KeyCommand::Generate => {
                    info!("Generating new identity...");

                    // Get the key path once and reuse it
                    let key_path = match mate::crypto::storage::default_key_path() {
                        Ok(path) => path,
                        Err(e) => {
                            error!("Failed to determine key storage path: {}", e);
                            return Ok(());
                        }
                    };

                    // Check if identity already exists
                    if key_path.exists() {
                        warn!("An identity already exists at: {}", key_path.display());
                        warn!("This will overwrite the existing identity!");
                    }

                    let identity = Identity::generate()?;
                    identity.save_to_default_storage()?;

                    info!("Identity generated successfully!");
                    info!("Peer ID: {}", identity.peer_id());
                    info!(
                        "Public Key: {}",
                        general_purpose::STANDARD.encode(identity.verifying_key().to_bytes())
                    );
                    info!("Saved to: {}", key_path.display());
                }
                KeyCommand::Info => {
                    info!("Showing identity information...");

                    match Identity::from_default_storage() {
                        Ok(identity) => {
                            info!("Peer ID: {}", identity.peer_id());
                            info!(
                                "Public Key: {}",
                                general_purpose::STANDARD
                                    .encode(identity.verifying_key().to_bytes())
                            );

                            if let Ok(path) = mate::crypto::storage::default_key_path() {
                                info!("Storage location: {}", path.display());
                            }
                        }
                        Err(e) => {
                            error!("No identity found: {}", e);
                            info!("Run 'mate key generate' to create a new identity");
                        }
                    }
                }
            }
        }
        Commands::Serve { bind } => {
            info!("Starting server on {}", bind);
            debug!("Server lifecycle: Initializing server components");

            // Use secure storage for identity
            let identity = std::sync::Arc::new(init_identity().await?);
            info!("Loaded identity: {}", identity.peer_id());
            debug!("Server lifecycle: Identity loaded successfully");

            // Create and run server with graceful shutdown handling
            let server = mate::network::Server::bind(&bind, identity).await?;

            info!("Server bound successfully, starting to accept connections...");
            debug!("Server lifecycle: Server bound, installing signal handlers");

            // Run server with graceful shutdown
            tokio::select! {
                result = server.run() => {
                    match result {
                        Ok(()) => {
                            info!("Server shutdown complete");
                            debug!("Server lifecycle: Server terminated normally");
                        }
                        Err(e) => {
                            error!("Server error: {}", e);
                            debug!("Server lifecycle: Server terminated with error");
                        }
                    }
                }
                _ = setup_shutdown_signal() => {
                    info!("Server lifecycle: Shutdown signal received, terminating server...");
                    debug!("Server lifecycle: Graceful shutdown initiated");

                    // Note: The server will automatically stop when this scope ends
                    // and the server variable is dropped, which closes all connections
                    if let Err(cleanup_error) = graceful_shutdown(None).await {
                        warn!("Server lifecycle: Cleanup encountered issues: {}", cleanup_error);
                    } else {
                        debug!("Server lifecycle: Cleanup completed successfully");
                    }

                    info!("Server lifecycle: Graceful shutdown completed");
                }
            }
        }
        Commands::Connect { address, message } => {
            info!("Connecting to {}", address);

            // Use secure storage for identity
            let identity = Arc::new(init_identity().await?);
            info!("Using identity: {}", identity.peer_id());

            // Create client instance
            let client = Client::new(identity);

            // Attempt connection
            match client.connect(&address).await {
                Ok(mut connection) => {
                    let peer_id = connection.peer_identity().unwrap_or("unknown").to_string();
                    info!("Connected to peer: {}", peer_id);

                    // Handle one-shot message mode
                    if let Some(msg_text) = message {
                        info!("Sending message: \"{}\"", msg_text);
                        let start_time = Instant::now();
                        let ping_message =
                            Message::new_ping(rand::random::<u64>(), msg_text.clone());

                        // Send message and measure round-trip time
                        match connection.send_message(ping_message).await {
                            Ok(()) => match connection.receive_message().await {
                                Ok((response, _sender)) => {
                                    let round_trip_time = start_time.elapsed();
                                    info!(
                                        "Received echo: \"{}\" (round-trip: {})",
                                        response.get_payload(),
                                        format_round_trip_time(round_trip_time)
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to receive response: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Failed to send message: {}", e);
                            }
                        }
                    } else {
                        // Interactive mode - enhanced session management with help commands and status display
                        println!("=== MATE Chat Session ===");
                        println!("Connected to peer: {}", peer_id);
                        println!("Connection status: Active");
                        println!();
                        println!("Available commands:");
                        println!("  help    - Show this help message");
                        println!("  info    - Show connection information");
                        println!("  quit    - Exit the chat session");
                        println!("  exit    - Exit the chat session");
                        println!();
                        println!("Type messages and press Enter to send. Press Ctrl+C or Ctrl+D to exit.");
                        println!("{}", "=".repeat(30));

                        let stdin = io::stdin();
                        let mut stdin_lock = stdin.lock();
                        let mut message_count = 0u32;
                        let mut total_round_trip_time = std::time::Duration::ZERO;
                        let session_start = Instant::now();

                        loop {
                            // Display prompt with connection status indicator
                            print!("mate> ");
                            io::stdout().flush().unwrap();

                            // Read user input
                            let mut input = String::new();
                            match stdin_lock.read_line(&mut input) {
                                Ok(0) => {
                                    // EOF (Ctrl+D)
                                    println!(); // New line after Ctrl+D
                                    break;
                                }
                                Ok(_) => {
                                    let input = input.trim().to_string();

                                    // Handle empty input
                                    if input.is_empty() {
                                        continue;
                                    }

                                    // Handle special commands
                                    match input.as_str() {
                                        "help" => {
                                            println!("=== Available Commands ===");
                                            println!("  help    - Show this help message");
                                            println!("  info    - Show connection information");
                                            println!("  quit    - Exit the chat session");
                                            println!("  exit    - Exit the chat session");
                                            println!();
                                            println!("Any other text will be sent as a message to the peer.");
                                            continue;
                                        }
                                        "info" => {
                                            let session_duration = session_start.elapsed();
                                            println!("=== Connection Information ===");
                                            println!("Peer ID: {}", peer_id);
                                            println!("Connection status: Active");
                                            println!(
                                                "Session duration: {}",
                                                format_round_trip_time(session_duration)
                                            );
                                            println!("Messages sent: {}", message_count);
                                            if message_count > 0 {
                                                let avg_time =
                                                    total_round_trip_time / message_count;
                                                println!(
                                                    "Average round-trip time: {}",
                                                    format_round_trip_time(avg_time)
                                                );
                                            }
                                            continue;
                                        }
                                        "quit" | "exit" => {
                                            break;
                                        }
                                        _ => {
                                            // Regular message - send to peer
                                        }
                                    }

                                    // Send message and measure round-trip time
                                    let start_time = Instant::now();
                                    let ping_message =
                                        Message::new_ping(rand::random::<u64>(), input.clone());

                                    match connection.send_message(ping_message).await {
                                        Ok(()) => {
                                            match connection.receive_message().await {
                                                Ok((response, _sender)) => {
                                                    let round_trip_time = start_time.elapsed();
                                                    message_count += 1;
                                                    total_round_trip_time += round_trip_time;
                                                    println!(
                                                        "← Received echo: \"{}\" (round-trip: {})",
                                                        response.get_payload(),
                                                        format_round_trip_time(round_trip_time)
                                                    );
                                                }
                                                Err(e) => {
                                                    error!("Connection error: Failed to receive response: {}", e);
                                                    warn!("The connection to the peer may have been lost.");
                                                    println!("Connection status: Disconnected");
                                                    println!("Attempting to reconnect...");

                                                    // Attempt to reconnect (basic retry logic)
                                                    match client.connect(&address).await {
                                                        Ok(new_connection) => {
                                                            connection = new_connection;
                                                            let new_peer_id = connection
                                                                .peer_identity()
                                                                .unwrap_or("unknown")
                                                                .to_string();
                                                            info!(
                                                                "Reconnected to peer: {}",
                                                                new_peer_id
                                                            );
                                                            println!(
                                                                "Connection status: Reconnected"
                                                            );
                                                        }
                                                        Err(reconnect_err) => {
                                                            error!(
                                                                "Failed to reconnect: {}",
                                                                reconnect_err
                                                            );
                                                            println!("Reconnection failed. Please restart the session.");
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Connection error: Failed to send message: {}",
                                                e
                                            );
                                            warn!("The connection to the peer may have been lost.");
                                            println!("Connection status: Disconnected");
                                            println!("Attempting to reconnect...");

                                            // Attempt to reconnect (basic retry logic)
                                            match client.connect(&address).await {
                                                Ok(new_connection) => {
                                                    connection = new_connection;
                                                    let new_peer_id = connection
                                                        .peer_identity()
                                                        .unwrap_or("unknown")
                                                        .to_string();
                                                    info!("Reconnected to peer: {}", new_peer_id);
                                                    println!("Connection status: Reconnected");

                                                    // Retry sending the message
                                                    let retry_start_time = Instant::now();
                                                    let retry_ping_message = Message::new_ping(
                                                        rand::random::<u64>(),
                                                        input.clone(),
                                                    );
                                                    match connection
                                                        .send_message(retry_ping_message)
                                                        .await
                                                    {
                                                        Ok(()) => {
                                                            match connection.receive_message().await
                                                            {
                                                                Ok((response, _sender)) => {
                                                                    let round_trip_time =
                                                                        retry_start_time.elapsed();
                                                                    message_count += 1;
                                                                    total_round_trip_time +=
                                                                        round_trip_time;
                                                                    println!("← Received echo: \"{}\" (round-trip: {})", 
                                                                            response.get_payload(), format_round_trip_time(round_trip_time));
                                                                }
                                                                Err(e) => {
                                                                    error!("Still failed to receive after reconnect: {}", e);
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Still failed to send after reconnect: {}", e);
                                                            break;
                                                        }
                                                    }
                                                }
                                                Err(reconnect_err) => {
                                                    error!(
                                                        "Failed to reconnect: {}",
                                                        reconnect_err
                                                    );
                                                    println!("Reconnection failed. Please restart the session.");
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to read input: {}", e);
                                    break;
                                }
                            }
                        }

                        // Display session summary
                        let session_duration = session_start.elapsed();
                        println!();
                        println!("=== Session Summary ===");
                        println!(
                            "Session duration: {}",
                            format_round_trip_time(session_duration)
                        );
                        if message_count > 0 {
                            let avg_time = total_round_trip_time / message_count;
                            println!("Messages sent: {}", message_count);
                            println!(
                                "Average round-trip time: {}",
                                format_round_trip_time(avg_time)
                            );
                        } else {
                            println!("No messages sent during this session");
                        }
                        println!("Goodbye!");
                    }

                    // Close connection
                    if let Err(e) = connection.close().await {
                        warn!("Failed to close connection cleanly: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", address, e);
                    std::process::exit(1);
                }
            }
        }

        // Chess commands - Initialize App once and handle all chess operations with proper lifecycle management
        Commands::Games
        | Commands::Board { .. }
        | Commands::Invite { .. }
        | Commands::Accept { .. }
        | Commands::Move { .. }
        | Commands::History { .. } => {
            info!("Initializing chess application...");
            debug!("Chess command lifecycle: Starting application initialization");

            // Initialize App instance once for all chess commands
            let app = App::new()
                .await
                .context("Failed to initialize application")?;

            info!("Chess application initialized successfully");
            debug!("Chess command lifecycle: Application initialization complete");
            debug!("Peer ID: {}", app.peer_id());
            debug!("Data directory: {}", app.data_dir().display());

            // Execute the chess command with proper lifecycle management
            let command_result = match cli.command {
                Commands::Games => {
                    info!("Chess command lifecycle: Starting games list operation");
                    debug!("Retrieving active games from database");

                    let result = app.handle_games().await.context("Failed to list games");

                    match &result {
                        Ok(()) => {
                            info!("Chess command lifecycle: Games list operation completed successfully");
                            debug!("Games list command finished without errors");
                        }
                        Err(e) => {
                            error!(
                                "Chess command lifecycle: Games list operation failed: {}",
                                e
                            );
                        }
                    }
                    result
                }

                Commands::Board { game_id } => {
                    if let Some(ref id) = game_id {
                        info!(
                            "Chess command lifecycle: Starting board display for game: {}",
                            id
                        );
                        debug!("Retrieving board state for specific game ID");
                    } else {
                        info!(
                            "Chess command lifecycle: Starting board display for most recent game"
                        );
                        debug!("Retrieving board state for most recent active game");
                    }

                    let result = app
                        .handle_board(game_id)
                        .await
                        .context("Failed to display board");

                    match &result {
                        Ok(()) => {
                            info!("Chess command lifecycle: Board display operation completed successfully");
                            debug!("Board display command finished without errors");
                        }
                        Err(e) => {
                            error!(
                                "Chess command lifecycle: Board display operation failed: {}",
                                e
                            );
                        }
                    }
                    result
                }

                Commands::Invite { address, color } => {
                    info!(
                        "Chess command lifecycle: Starting game invitation to: {}",
                        address
                    );
                    if let Some(ref color_pref) = color {
                        debug!("Color preference specified: {}", color_pref);
                    } else {
                        debug!("No color preference specified, will use random selection");
                    }

                    let result = app
                        .handle_invite(address, color)
                        .await
                        .context("Failed to send invitation");

                    match &result {
                        Ok(()) => {
                            info!("Chess command lifecycle: Game invitation sent successfully");
                            debug!("Invitation command finished without errors");
                        }
                        Err(e) => {
                            error!("Chess command lifecycle: Game invitation failed: {}", e);
                        }
                    }
                    result
                }

                Commands::Accept { game_id, color } => {
                    info!(
                        "Chess command lifecycle: Starting game acceptance for: {}",
                        game_id
                    );
                    if let Some(ref color_pref) = color {
                        debug!("Color preference specified: {}", color_pref);
                    } else {
                        debug!("No color preference specified, will use automatic selection");
                    }

                    let result = app
                        .handle_accept(game_id, color)
                        .await
                        .context("Failed to accept invitation");

                    match &result {
                        Ok(()) => {
                            info!("Chess command lifecycle: Game invitation accepted successfully");
                            debug!("Accept command finished without errors");
                        }
                        Err(e) => {
                            error!("Chess command lifecycle: Game acceptance failed: {}", e);
                        }
                    }
                    result
                }

                Commands::Move {
                    chess_move,
                    game_id,
                } => {
                    if let Some(ref id) = game_id {
                        info!(
                            "Chess command lifecycle: Starting move '{}' in game: {}",
                            chess_move, id
                        );
                        debug!("Making move in specific game ID");
                    } else {
                        info!(
                            "Chess command lifecycle: Starting move '{}' in most recent game",
                            chess_move
                        );
                        debug!("Making move in most recent active game");
                    }

                    let result = app
                        .handle_move(game_id, chess_move)
                        .await
                        .context("Failed to make move");

                    match &result {
                        Ok(()) => {
                            info!("Chess command lifecycle: Move executed successfully");
                            debug!("Move command finished without errors");
                        }
                        Err(e) => {
                            error!("Chess command lifecycle: Move execution failed: {}", e);
                        }
                    }
                    result
                }

                Commands::History { game_id } => {
                    if let Some(ref id) = game_id {
                        info!(
                            "Chess command lifecycle: Starting history display for game: {}",
                            id
                        );
                        debug!("Retrieving move history for specific game ID");
                    } else {
                        info!("Chess command lifecycle: Starting history display for most recent game");
                        debug!("Retrieving move history for most recent active game");
                    }

                    let result = app
                        .handle_history(game_id)
                        .await
                        .context("Failed to show game history");

                    match &result {
                        Ok(()) => {
                            info!(
                                "Chess command lifecycle: History display completed successfully"
                            );
                            debug!("History command finished without errors");
                        }
                        Err(e) => {
                            error!("Chess command lifecycle: History display failed: {}", e);
                        }
                    }
                    result
                }

                _ => unreachable!("Non-chess commands should not reach this branch"),
            };

            // Ensure graceful cleanup regardless of command result
            debug!("Chess command lifecycle: Starting cleanup phase");
            if let Err(cleanup_error) = graceful_shutdown(Some(&app)).await {
                warn!(
                    "Chess command lifecycle: Cleanup encountered issues: {}",
                    cleanup_error
                );
            } else {
                debug!("Chess command lifecycle: Cleanup completed successfully");
            }

            // Return the command result
            command_result?;
            info!("Chess command lifecycle: Operation completed successfully");
        }
    }

    debug!("Application lifecycle: All operations completed successfully");
    info!("Mate application finished successfully");
    Ok(())
}
