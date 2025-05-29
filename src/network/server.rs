use crate::crypto::Identity;
use tokio::net::TcpListener;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::HashMap;

// Step 2.1: Add Required Imports
// Add wire protocol imports
use crate::messages::wire::{WireConfig, WireProtocolError};
use crate::network::{Connection, ConnectionError};
// Add async handling imports
use tokio::task::{self, JoinHandle};
use tracing::{info, error, warn, debug, instrument};

pub struct Server {
    identity: Arc<Identity>,
    listener: TcpListener,
    wire_config: WireConfig,
}

impl Server {
    pub async fn bind(addr: &str, identity: Arc<Identity>) -> Result<Self> {
        // Bind TcpListener to the provided address
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind server to address: {}", addr))?;
        
        // Initialize with default WireConfig
        let wire_config = WireConfig::default();
        
        // Log successful server binding
        info!("Server successfully bound to address: {}", addr);
        
        Ok(Self {
            identity,
            listener,
            wire_config,
        })
    }
    
    /// Create a server with custom wire configuration
    pub async fn bind_with_config(
        addr: &str, 
        identity: Arc<Identity>, 
        wire_config: WireConfig
    ) -> Result<Self> {
        // Bind TcpListener to the provided address
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind server to address: {}", addr))?;
        
        // Log successful server binding with custom config
        info!("Server successfully bound to address: {} with custom wire config", addr);
        debug!("Wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}", 
               wire_config.max_message_size, wire_config.read_timeout, wire_config.write_timeout);
        
        Ok(Self {
            identity,
            listener,
            wire_config,
        })
    }
    
    pub async fn run(self) -> Result<()> {
        info!("Starting server on address: {:?}", self.listener.local_addr()?);
        
        // Track active connections for management
        let mut active_connections: HashMap<usize, JoinHandle<()>> = HashMap::new();
        let mut connection_counter = 0usize;
        
        loop {
            // Accept incoming connections
            match self.listener.accept().await {
                Ok((stream, peer_addr)) => {
                    connection_counter += 1;
                    let connection_id = connection_counter;
                    
                    info!("Accepted new connection {} from {}", connection_id, peer_addr);
                    
                    // Check connection limits
                    const MAX_CONNECTIONS: usize = 1000; // TODO: Make this configurable
                    if active_connections.len() >= MAX_CONNECTIONS {
                        warn!("Connection limit reached ({}), rejecting connection from {}", 
                              MAX_CONNECTIONS, peer_addr);
                        // Stream will be dropped, closing the connection
                        continue;
                    }
                    
                    // Clone necessary data for the spawned task
                    let identity = Arc::clone(&self.identity);
                    let wire_config = self.wire_config.clone();
                    
                    // Spawn async task for each connection
                    let handle = task::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, identity, wire_config, connection_id).await {
                            error!("Connection {} failed: {}", connection_id, e);
                        } else {
                            info!("Connection {} completed successfully", connection_id);
                        }
                    });
                    
                    // Track the connection
                    active_connections.insert(connection_id, handle);
                    
                    // Clean up completed connections
                    active_connections.retain(|id, handle| {
                        if handle.is_finished() {
                            debug!("Cleaning up completed connection {}", id);
                            false
                        } else {
                            true
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    // Continue accepting other connections despite this error
                    continue;
                }
            }
        }
    }
    
    /// Handle individual connection lifecycle
    #[instrument(skip(stream, identity, wire_config), fields(connection_id = connection_id))]
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        identity: Arc<Identity>, 
        wire_config: WireConfig,
        connection_id: usize
    ) -> Result<()> {
        info!("Handling connection {}", connection_id);
        
        // Create Connection with wire protocol
        let mut connection = Connection::new_with_config(stream, identity, wire_config).await;
        
        // Perform handshake
        let peer_id = match connection.handshake().await {
            Ok(peer_id) => {
                info!("Handshake successful for connection {} with peer: {}", connection_id, peer_id);
                peer_id
            }
            Err(e) => {
                error!("Handshake failed for connection {}: {}", connection_id, e);
                return Err(e.into());
            }
        };
        
        // Message processing loop
        loop {
            match connection.receive_message().await {
                Ok((message, sender)) => {
                    info!("Received {} message from {} on connection {}", 
                          message.message_type(), sender, connection_id);
                    
                    // Handle different message types
                    match message.message_type() {
                        "ping" => {
                            // Echo the ping message back
                            debug!("Echoing ping message back to {}", sender);
                            if let Err(e) = connection.send_message(message).await {
                                error!("Failed to echo message on connection {}: {}", connection_id, e);
                                break;
                            }
                        }
                        _ => {
                            debug!("Received {} message from {} (no specific handler)", 
                                   message.message_type(), sender);
                            // For now, just log other message types
                            // TODO: Add proper message routing and handling
                        }
                    }
                }
                Err(e) => {
                    match e {
                        ConnectionError::WireProtocol(WireProtocolError::ReadTimeout { .. }) => {
                            debug!("Read timeout on connection {}, closing", connection_id);
                        }
                        ConnectionError::ConnectionClosed => {
                            info!("Connection {} closed by peer", connection_id);
                        }
                        _ => {
                            error!("Error receiving message on connection {}: {}", connection_id, e);
                        }
                    }
                    break;
                }
            }
        }
        
        // Connection cleanup
        if let Err(e) = connection.close().await {
            warn!("Error during connection {} cleanup: {}", connection_id, e);
        } else {
            info!("Connection {} closed cleanly", connection_id);
        }
        
        Ok(())
    }
}
