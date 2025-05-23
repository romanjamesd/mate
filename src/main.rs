use mate::cli::{Cli, Commands};
use mate::crypto::Identity;
use clap::Parser;
use anyhow::Result;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init => {
            info!("Initializing new identity...");
            // Implementation placeholder
            todo!()
        }
        Commands::Info => {
            info!("Showing identity information...");
            // Implementation placeholder
            todo!()
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
}
