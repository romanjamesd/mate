use crate::crypto::Identity;
use tokio::net::TcpListener;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::HashMap;

// Step 2.1: Add Required Imports
// Add wire protocol imports
use crate::messages::wire::{WireConfig, WireProtocolError, SERVER_MAX_CONCURRENT_CONNECTIONS};
use crate::network::{Connection, ConnectionError};
// Add async handling imports
use tokio::task::{self, JoinHandle};
use tracing::{info, error, warn, debug, instrument};

// Step 4.2: Error conversion is automatically provided by anyhow's blanket implementation
// since ConnectionError and WireProtocolError implement std::error::Error via thiserror::Error

/// A secure peer-to-peer server with integrated authentication and wire protocol support.
/// 
/// The `Server` provides a robust foundation for accepting and managing multiple concurrent
/// peer connections. Each connection is automatically authenticated through the handshake
/// protocol and maintains cryptographic message integrity.
/// 
/// # Security Features
/// 
/// - **Automatic Authentication**: All incoming connections must complete handshake protocol
/// - **Connection Limits**: Built-in protection against connection exhaustion attacks
/// - **Cryptographic Verification**: All messages are automatically verified for authenticity
/// - **Isolation**: Each connection is handled in a separate async task for security isolation
/// 
/// # Error Recovery Strategies
/// 
/// ## Server-Level Errors
/// - **Bind failures**: Usually indicate port conflicts or permission issues.
///   - *Recovery*: Check port availability, verify permissions, try alternative ports.
/// - **Accept failures**: Individual connection accept errors don't stop the server.
///   - *Recovery*: Logged and server continues accepting other connections.
/// 
/// ## Connection-Level Error Handling
/// - **Handshake failures**: Connections with failed handshakes are automatically closed.
///   - *Recovery*: Connection is terminated, server continues operating normally.
/// - **Message errors**: Invalid messages don't affect other connections.
///   - *Recovery*: Error logged, connection may be closed depending on severity.
/// - **Connection limits exceeded**: New connections are rejected when limit reached.
///   - *Recovery*: Client should retry after delay, server remains stable.
/// 
/// ## Performance Monitoring
/// - Connection counts and lifetimes are automatically logged
/// - Failed handshakes and connection errors are tracked
/// - Performance metrics available in debug logs
/// 
/// # Configuration Options
/// 
/// ## Wire Protocol Configuration
/// - **Message size limits**: Configure via `WireConfig::max_message_size`
/// - **Timeout values**: Adjust read/write timeouts for network conditions
/// - **Handshake timeout**: Fixed at 10 seconds for security
/// 
/// ## Connection Management
/// - **Concurrent connection limit**: Set via `SERVER_MAX_CONCURRENT_CONNECTIONS`
/// - **Automatic cleanup**: Completed connections are cleaned up automatically
/// 
/// # Example Usage
/// 
/// ```rust
/// use std::sync::Arc;
/// 
/// // Basic server setup
/// let server = Server::bind("0.0.0.0:8080", identity).await?;
/// 
/// // Start accepting connections (runs forever)
/// server.run().await?;
/// 
/// // With custom configuration
/// let custom_config = WireConfig::for_server()
///     .with_max_message_size(1024 * 1024); // 1MB messages
/// let server = Server::bind_with_config("0.0.0.0:8080", identity, custom_config).await?;
/// ```
/// 
/// # Thread Safety
/// 
/// The server is designed to be run in a single async task. Connection handling is automatically
/// distributed across the tokio runtime's thread pool.
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
        
        // Initialize with Step 5.1 server-optimized WireConfig
        let wire_config = WireConfig::for_server();
        
        // Log successful server binding
        info!("Server successfully bound to address: {} with server-optimized configuration", addr);
        debug!("Server wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}", 
               wire_config.max_message_size, wire_config.read_timeout, wire_config.write_timeout);
        
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
                    
                    // Check connection limits using Step 5.1 configuration constants
                    if active_connections.len() >= SERVER_MAX_CONCURRENT_CONNECTIONS {
                        warn!("Connection limit reached ({}), rejecting connection from {}", 
                              SERVER_MAX_CONCURRENT_CONNECTIONS, peer_addr);
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
        let _peer_id = match connection.handle_handshake_request().await {
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
