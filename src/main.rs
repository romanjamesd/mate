use mate::cli::{Cli, Commands, KeyCommand};
use mate::crypto::Identity;
use clap::Parser;
use anyhow::Result;
use tracing::{info, error, warn};
use base64::{Engine as _, engine::general_purpose};

/// Initialize identity using secure storage
pub async fn init_identity() -> Result<Identity> {
    Identity::load_or_generate()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
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
            let identity = init_identity().await?;
            info!("Loaded identity: {}", identity.peer_id());
            
            // Implementation placeholder
            todo!()
        }
        Commands::Connect { address, message } => {
            info!("Connecting to {}", address);
            
            // Use secure storage for identity
            let identity = init_identity().await?;
            info!("Using identity: {}", identity.peer_id());
            
            // Implementation placeholder
            todo!()
        }
    }
    
    Ok(())
}
