use mate::cli::{Cli, Commands, KeyCommand};
use mate::crypto::Identity;
use mate::network::Client;
use mate::messages::Message;
use clap::Parser;
use anyhow::Result;
use tracing::{info, error, warn};
use base64::{Engine as _, engine::general_purpose};
use tokio::time::Instant;
use std::sync::Arc;
use std::io::{self, Write, BufRead};
use rand;

/// Format round-trip time for display with appropriate precision
fn format_round_trip_time(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    let micros = duration.as_micros();
    
    if millis == 0 {
        format!("{}μs", micros)
    } else if millis < 1000 {
        format!("{}ms", millis)
    } else {
        let seconds = duration.as_secs_f64();
        format!("{:.2}s", seconds)
    }
}

/// Initialize identity using secure storage
pub async fn init_identity() -> Result<Identity> {
    Identity::load_or_generate()
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
                .from_env_lossy()
        )
        .with_target(false)  // Hide target module in logs for cleaner output
        .with_level(true)    // Show log levels
        .with_file(false)    // Hide file names for production
        .with_line_number(false) // Hide line numbers for production
        .init();
    
    info!("Starting mate application with network-optimized logging configuration");
    
    let cli = Cli::parse();
    
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
                    info!("Public Key: {}", general_purpose::STANDARD.encode(identity.verifying_key().to_bytes()));
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
                    info!("Public Key: {}", general_purpose::STANDARD.encode(identity.verifying_key().to_bytes()));
                    info!("Saved to: {}", key_path.display());
                }
                KeyCommand::Info => {
                    info!("Showing identity information...");
                    
                    match Identity::from_default_storage() {
                        Ok(identity) => {
                            info!("Peer ID: {}", identity.peer_id());
                            info!("Public Key: {}", general_purpose::STANDARD.encode(identity.verifying_key().to_bytes()));
                            
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
            
            // Use secure storage for identity
            let identity = std::sync::Arc::new(init_identity().await?);
            info!("Loaded identity: {}", identity.peer_id());
            
            // Create and run server
            let server = mate::network::Server::bind(&bind, identity).await?;
            
            info!("Server bound successfully, starting to accept connections...");
            if let Err(e) = server.run().await {
                error!("Server error: {}", e);
                std::process::exit(1);
            }
            
            info!("Server shutdown complete");
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
                    let peer_id = connection.peer_identity()
                        .unwrap_or("unknown")
                        .to_string();
                    info!("Connected to peer: {}", peer_id);
                    
                    // Handle one-shot message mode
                    if let Some(msg_text) = message {
                        info!("Sending message: \"{}\"", msg_text);
                        let start_time = Instant::now();
                        let ping_message = Message::new_ping(rand::random::<u64>(), msg_text.clone());
                        
                        // Send message and measure round-trip time
                        match connection.send_message(ping_message).await {
                            Ok(()) => {
                                match connection.receive_message().await {
                                    Ok((response, _sender)) => {
                                        let round_trip_time = start_time.elapsed();
                                        info!("Received echo: \"{}\" (round-trip: {})", 
                                              response.get_payload(), format_round_trip_time(round_trip_time));
                                    }
                                    Err(e) => {
                                        error!("Failed to receive response: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to send message: {}", e);
                            }
                        }
                    } else {
                        // Interactive mode - full implementation with enhanced timing statistics
                        info!("Connected to peer: {}", peer_id);
                        info!("Interactive mode - type messages and press Enter. Type 'quit' to exit.");
                        
                        let stdin = io::stdin();
                        let mut stdin_lock = stdin.lock();
                        let mut message_count = 0u32;
                        let mut total_round_trip_time = std::time::Duration::ZERO;
                        
                        loop {
                            // Display prompt
                            print!("> ");
                            io::stdout().flush().unwrap();
                            
                            // Read user input
                            let mut input = String::new();
                            match stdin_lock.read_line(&mut input) {
                                Ok(0) => {
                                    // EOF (Ctrl+D)
                                    if message_count > 0 {
                                        let avg_time = total_round_trip_time / message_count;
                                        info!("Session summary: {} messages sent, average round-trip: {}", 
                                              message_count, format_round_trip_time(avg_time));
                                    }
                                    info!("Goodbye!");
                                    break;
                                }
                                Ok(_) => {
                                    let input = input.trim().to_string();
                                    
                                    // Handle quit command
                                    if input == "quit" || input == "exit" {
                                        if message_count > 0 {
                                            let avg_time = total_round_trip_time / message_count;
                                            info!("Session summary: {} messages sent, average round-trip: {}", 
                                                  message_count, format_round_trip_time(avg_time));
                                        }
                                        info!("Goodbye!");
                                        break;
                                    }
                                    
                                    // Skip empty messages
                                    if input.is_empty() {
                                        continue;
                                    }
                                    
                                    // Send message and measure round-trip time
                                    let start_time = Instant::now();
                                    let ping_message = Message::new_ping(rand::random::<u64>(), input.clone());
                                    
                                    match connection.send_message(ping_message).await {
                                        Ok(()) => {
                                            match connection.receive_message().await {
                                                Ok((response, _sender)) => {
                                                    let round_trip_time = start_time.elapsed();
                                                    message_count += 1;
                                                    total_round_trip_time += round_trip_time;
                                                    info!("Received echo: \"{}\" (round-trip: {})", 
                                                          response.get_payload(), format_round_trip_time(round_trip_time));
                                                }
                                                Err(e) => {
                                                    error!("Failed to receive response: {}", e);
                                                    error!("Connection may have been lost");
                                                    break;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to send message: {}", e);
                                            error!("Connection may have been lost");
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to read input: {}", e);
                                    break;
                                }
                            }
                        }
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
    }
    
    Ok(())
}
