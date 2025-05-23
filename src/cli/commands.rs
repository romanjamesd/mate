use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mate")]
#[command(about = "A P2P echo server for testing")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new identity
    Init,
    /// Show current peer ID and identity info
    Info,
    /// Start the echo server
    Serve {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Connect to a peer
    Connect {
        /// Address to connect to
        address: String,
        #[arg(short, long)]
        message: Option<String>,
    },
}
