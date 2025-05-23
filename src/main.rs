use mate::cli::{Cli, Commands};
use mate::crypto::Identity;
use clap::Parser;
use anyhow::Result;
use tracing::{info, error};
use std::path::PathBuf;
use base64::{Engine as _, engine::general_purpose};

const IDENTITY_FILE: &str = "identity.json";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init => {
            info!("Initializing new identity...");
            
            let identity_path = PathBuf::from(IDENTITY_FILE);
            if identity_path.exists() {
                error!("Identity file already exists at {}", identity_path.display());
                return Ok(());
            }
            
            let identity = Identity::generate()?;
            identity.save_to_file(&identity_path)?;
            
            info!("Identity created successfully!");
            info!("Peer ID: {}", identity.peer_id());
            info!("Saved to: {}", identity_path.display());
        }
        Commands::Info => {
            info!("Showing identity information...");
            
            let identity_path = PathBuf::from(IDENTITY_FILE);
            if !identity_path.exists() {
                error!("No identity found. Run 'init' first.");
                return Ok(());
            }
            
            let identity = Identity::from_file(&identity_path)?;
            info!("Peer ID: {}", identity.peer_id());
            info!("Public Key: {}", general_purpose::STANDARD.encode(identity.verifying_key().to_bytes()));
        }
        Commands::Serve { bind } => {
            info!("Starting server on {}", bind);
            // Implementation placeholder
            todo!()
        }
        Commands::Connect { address, message } => {
            info!("Connecting to {}", address);
            // Implementation placeholder
            todo!()
        }
    }
    
    Ok(())
}
