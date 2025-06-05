use crate::crypto::Identity;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;

// Step 2.1: Add Required Imports
// Add wire protocol imports
use crate::messages::wire::{WireConfig, WireProtocolError, SERVER_MAX_CONCURRENT_CONNECTIONS};
use crate::network::{Connection, ConnectionError};
// Add async handling imports
use tokio::task::{self, JoinHandle};
use tracing::{debug, error, info, instrument, warn};

// Step 3: Shutdown communication imports
use tokio::sync::broadcast;

// Step 4: Graceful shutdown imports
use std::time::Duration;

// Step 2: Signal handling imports for graceful shutdown
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

#[cfg(windows)]
use tokio::signal;

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
/// # Graceful Shutdown
///
/// The server supports graceful shutdown through signal handling:
/// - **SIGTERM/SIGINT**: Initiates graceful shutdown sequence
/// - **Connection Cleanup**: Active connections are given time to complete
/// - **Timeout Handling**: Connections are force-closed after 30-second timeout
/// - **Resource Cleanup**: All server resources are properly released
///
/// ## Shutdown Process
/// 1. **Signal Detection**: Server monitors for SIGTERM, SIGINT, and Ctrl+C
/// 2. **Stop Accepting**: New connections are immediately rejected
/// 3. **Notify Connections**: All active connections receive shutdown signals
/// 4. **Graceful Wait**: Up to 30 seconds for connections to complete naturally
/// 5. **Force Close**: Remaining connections are terminated after timeout
/// 6. **Resource Cleanup**: All server resources are properly released
///
/// # Example Usage
///
/// ```rust
/// use std::sync::Arc;
/// use mate::network::Server;
/// use mate::crypto::Identity;
/// use mate::messages::wire::WireConfig;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create test identity
///     let identity = Arc::new(Identity::generate()?);
///     
///     // Basic server setup
///     let server = Server::bind("0.0.0.0:8080", identity.clone()).await?;
///     
///     // Start accepting connections (runs forever)
///     // server.run().await?;  // Commented out as this would run indefinitely
///     
///     // With custom configuration
///     let custom_config = WireConfig::with_max_message_size(1024 * 1024); // 1MB messages
///     let server = Server::bind_with_config("0.0.0.0:8081", identity, custom_config).await?;
///     
///     Ok(())
/// }
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
        info!(
            "Server successfully bound to address: {} with server-optimized configuration",
            addr
        );
        debug!(
            "Server wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}",
            wire_config.max_message_size, wire_config.read_timeout, wire_config.write_timeout
        );

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
        wire_config: WireConfig,
    ) -> Result<Self> {
        // Bind TcpListener to the provided address
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind server to address: {}", addr))?;

        // Log successful server binding with custom config
        info!(
            "Server successfully bound to address: {} with custom wire config",
            addr
        );
        debug!(
            "Wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}",
            wire_config.max_message_size, wire_config.read_timeout, wire_config.write_timeout
        );

        Ok(Self {
            identity,
            listener,
            wire_config,
        })
    }

    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn run(self) -> Result<()> {
        info!(
            "Starting server on address: {:?}",
            self.listener.local_addr()?
        );

        // Create shutdown broadcast channel for distributing signals to connections
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

        // Track active connections for management
        let mut active_connections: HashMap<usize, JoinHandle<()>> = HashMap::new();
        let mut connection_counter = 0usize;

        // Spawn shutdown signal handler
        let shutdown_handle = {
            let shutdown_tx = shutdown_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::wait_for_shutdown().await {
                    error!("Error in shutdown handler: {}", e);
                }
                let _ = shutdown_tx.send(());
            })
        };

        // Main server loop with shutdown handling
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping accept loop");
                    break;
                }

                // Accept new connections
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            connection_counter += 1;
                            let connection_id = connection_counter;

                            info!("Accepted new connection {} from {}", connection_id, peer_addr);

                            // Check connection limits
                            if active_connections.len() >= SERVER_MAX_CONCURRENT_CONNECTIONS {
                                warn!("Connection limit reached ({}), rejecting connection from {}",
                                      SERVER_MAX_CONCURRENT_CONNECTIONS, peer_addr);
                                continue;
                            }

                            // Clone necessary data for the spawned task
                            let identity = Arc::clone(&self.identity);
                            let wire_config = self.wire_config.clone();
                            let shutdown_rx = shutdown_tx.subscribe(); // Create subscriber for connection

                            // Spawn async task for each connection with shutdown support
                            let handle = task::spawn(async move {
                                if let Err(e) = Self::handle_connection_with_shutdown(
                                    stream, identity, wire_config, connection_id, shutdown_rx
                                ).await {
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
                            continue;
                        }
                    }
                }
            }
        }

        // Graceful shutdown process
        info!("Performing graceful shutdown...");
        self.graceful_shutdown(active_connections, shutdown_handle)
            .await?;

        Ok(())
    }

    /// Handle individual connection lifecycle with shutdown support
    #[instrument(skip(stream, identity, wire_config, shutdown_rx), fields(connection_id = connection_id))]
    async fn handle_connection_with_shutdown(
        stream: tokio::net::TcpStream,
        identity: Arc<Identity>,
        wire_config: WireConfig,
        connection_id: usize,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Handling connection {}", connection_id);

        // Create Connection with wire protocol
        let mut connection = Connection::new_with_config(stream, identity, wire_config).await;

        // Perform handshake
        let _peer_id = match connection.handle_handshake_request().await {
            Ok(peer_id) => {
                info!(
                    "Handshake successful for connection {} with peer: {}",
                    connection_id, peer_id
                );
                peer_id
            }
            Err(e) => {
                error!("Handshake failed for connection {}: {}", connection_id, e);
                return Err(e);
            }
        };

        // Message processing loop with shutdown handling
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, closing connection {}", connection_id);
                    break;
                }

                // Process messages
                result = connection.receive_message() => {
                    match result {
                        Ok((message, sender)) => {
                            info!("Received {} message from {} on connection {}",
                                  message.message_type(), sender, connection_id);

                            // Handle different message types
                            match message.message_type() {
                                "Ping" => {
                                    debug!("Echoing ping message back to {}", sender);
                                    if let Err(e) = connection.send_message(message).await {
                                        error!("Failed to echo message on connection {}: {}", connection_id, e);
                                        break;
                                    }
                                }
                                _ => {
                                    debug!("Received {} message from {} (no specific handler)",
                                           message.message_type(), sender);
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

    /// Wait for shutdown signals (SIGTERM, SIGINT)
    async fn wait_for_shutdown() -> Result<()> {
        #[cfg(unix)]
        {
            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, initiating graceful shutdown");
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
                }
            }
        }

        #[cfg(windows)]
        {
            match signal::ctrl_c().await {
                Ok(()) => info!("Received Ctrl+C, initiating graceful shutdown"),
                Err(e) => error!("Failed to listen for ctrl-c signal: {}", e),
            }
        }

        Ok(())
    }

    /// Perform graceful shutdown of the server
    async fn graceful_shutdown(
        &self,
        mut active_connections: HashMap<usize, JoinHandle<()>>,
        shutdown_handle: JoinHandle<()>,
    ) -> Result<()> {
        info!("Shutting down server gracefully...");

        // Stop accepting new connections (listener is dropped automatically)
        info!("Stopped accepting new connections");

        // Wait for active connections to complete (with timeout)
        let shutdown_timeout = Duration::from_secs(30);
        let start_time = std::time::Instant::now();

        info!(
            "Waiting for {} active connections to complete",
            active_connections.len()
        );

        while !active_connections.is_empty() && start_time.elapsed() < shutdown_timeout {
            // Check for completed connections
            active_connections.retain(|id, handle| {
                if handle.is_finished() {
                    info!("Connection {} completed during shutdown", id);
                    false
                } else {
                    true
                }
            });

            if !active_connections.is_empty() {
                debug!("Still waiting for {} connections", active_connections.len());
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Force close any remaining connections after timeout
        if !active_connections.is_empty() {
            warn!(
                "Forcing closure of {} remaining connections after timeout",
                active_connections.len()
            );

            for (id, handle) in active_connections {
                handle.abort();
                warn!("Force-closed connection {}", id);
            }
        }

        // Clean up shutdown handler
        shutdown_handle.abort();

        info!("Server shutdown complete");
        Ok(())
    }
}
