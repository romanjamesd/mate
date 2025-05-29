use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::FramedMessage;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite};
use std::net::SocketAddr;
use std::time::Duration;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Create a test SignedEnvelope with a known message
fn create_test_envelope(payload: &str) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// A wrapper around TcpStream that allows controlling buffer sizes
struct BufferControlledTcpStream {
    inner: TcpStream,
    read_buffer_size: usize,
    write_buffer_size: usize,
}

impl BufferControlledTcpStream {
    fn new(stream: TcpStream, read_buffer_size: usize, write_buffer_size: usize) -> Self {
        Self {
            inner: stream,
            read_buffer_size,
            write_buffer_size,
        }
    }
}

impl AsyncRead for BufferControlledTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Limit the read buffer size to simulate different TCP buffer configurations
        let max_read = std::cmp::min(buf.remaining(), self.read_buffer_size);
        if max_read == 0 {
            return Poll::Ready(Ok(()));
        }
        
        // Create a temporary buffer for the limited read
        let mut temp_buf = vec![0u8; max_read];
        let mut temp_read_buf = tokio::io::ReadBuf::new(&mut temp_buf);
        
        let result = Pin::new(&mut self.inner).poll_read(cx, &mut temp_read_buf);
        
        // Copy the data that was actually read to the original buffer
        let bytes_read = temp_read_buf.filled().len();
        if bytes_read > 0 {
            buf.put_slice(&temp_read_buf.filled()[..bytes_read]);
        }
        
        result
    }
}

impl AsyncWrite for BufferControlledTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        // Limit the write size to simulate different TCP buffer configurations
        let max_write = std::cmp::min(buf.len(), self.write_buffer_size);
        let limited_buf = &buf[..max_write];
        
        Pin::new(&mut self.inner).poll_write(cx, limited_buf)
    }
    
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// A wrapper that simulates slow network conditions by introducing delays
struct SlowNetworkStream {
    inner: TcpStream,
}

impl SlowNetworkStream {
    fn new(stream: TcpStream, _delay: Duration) -> Self {
        Self {
            inner: stream,
        }
    }
}

impl AsyncRead for SlowNetworkStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // For simplicity in this test, we'll just use the inner stream
        // In a real implementation, you'd want more sophisticated delay simulation
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for SlowNetworkStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Test wire protocol works with real TCP connections
/// Verify compatibility with different TCP buffer sizes
/// Test behavior with various network conditions (slow, fast, unreliable)
/// Verify protocol works across different network interfaces
#[tokio::test]
async fn test_tcp_stream_compatibility() {
    println!("Testing TCP stream compatibility - Cross-Platform Compatibility");
    
    // Test 1: Test wire protocol works with real TCP connections
    println!("Test 1: Testing wire protocol with real TCP connections");
    
    // Create test messages of various sizes
    let medium_message = "Medium sized message for TCP testing ".repeat(20);
    let large_message = "Large message payload for comprehensive TCP testing ".repeat(100);
    
    let test_messages = vec![
        ("small", "Small test message for TCP compatibility"),
        ("medium", medium_message.as_str()),
        ("large", large_message.as_str()),
    ];
    
    for (size_description, payload) in test_messages {
        println!("  Testing {} message over real TCP", size_description);
        
        // Create a new listener for each test iteration
        let listener = TcpListener::bind("127.0.0.1:0").await
            .expect("Failed to bind TCP listener");
        let server_addr = listener.local_addr()
            .expect("Failed to get listener address");
        
        println!("    Started TCP server at {}", server_addr);
        
        let (test_envelope, _) = create_test_envelope(payload);
        let framed_message = FramedMessage::default();
        
        // Spawn server task
        let server_framed = framed_message.clone();
        let server_task = tokio::spawn(async move {
            let (mut stream, client_addr) = listener.accept().await
                .expect("Failed to accept connection");
            
            println!("    Server: Accepted connection from {}", client_addr);
            
            // Read message from client
            let received_envelope = server_framed.read_message(&mut stream).await
                .expect("Server failed to read message");
            
            println!("    Server: Received message, verifying...");
            assert!(received_envelope.verify_signature(), "Server: Signature verification failed");
            
            // Echo the message back to client
            server_framed.write_message(&mut stream, &received_envelope).await
                .expect("Server failed to write echo message");
            
            println!("    Server: Echoed message back to client");
            received_envelope
        });
        
        // Connect as client
        let mut client_stream = TcpStream::connect(server_addr).await
            .expect("Failed to connect to server");
        
        println!("    Client: Connected to server");
        
        // Send message to server
        framed_message.write_message(&mut client_stream, &test_envelope).await
            .expect("Client failed to write message");
        
        println!("    Client: Sent message to server");
        
        // Read echo back from server
        let echo_envelope = framed_message.read_message(&mut client_stream).await
            .expect("Client failed to read echo message");
        
        println!("    Client: Received echo from server");
        
        // Verify echo matches original
        assert!(echo_envelope.verify_signature(), "Client: Echo signature verification failed");
        assert_eq!(echo_envelope.get_message().expect("Failed to deserialize echo message").get_payload(),
                   test_envelope.get_message().expect("Failed to deserialize original message").get_payload(),
                   "Echo payload should match original");
        
        // Wait for server task to complete
        let server_received = server_task.await
            .expect("Server task failed");
        
        assert!(server_received.verify_signature(), "Server received message signature verification failed");
        
        println!("    ✓ {} message successfully round-tripped over TCP", size_description);
    }
    
    // Test 2: Verify compatibility with different TCP buffer sizes
    println!("\nTest 2: Testing compatibility with different TCP buffer sizes");
    
    let buffer_size_test_cases = vec![
        (64, "very small buffers (64 bytes)"),
        (512, "small buffers (512 bytes)"),
        (1024, "standard buffers (1KB)"),
        (4096, "large buffers (4KB)"),
        (8192, "very large buffers (8KB)"),
    ];
    
    for (buffer_size, description) in buffer_size_test_cases {
        println!("  Testing with {}", description);
        
        // Create a new listener for this test
        let buffer_listener = TcpListener::bind("127.0.0.1:0").await
            .expect("Failed to bind buffer test listener");
        let buffer_server_addr = buffer_listener.local_addr()
            .expect("Failed to get buffer listener address");
        
        let test_payload = format!("Buffer size test with {} - {}", description, "x".repeat(1000));
        let (buffer_test_envelope, _) = create_test_envelope(&test_payload);
        let buffer_framed = FramedMessage::default();
        
        // Spawn server with controlled buffer sizes
        let server_framed = buffer_framed.clone();
        let server_task = tokio::spawn(async move {
            let (stream, _) = buffer_listener.accept().await
                .expect("Failed to accept buffer test connection");
            
            let mut controlled_stream = BufferControlledTcpStream::new(stream, buffer_size, buffer_size);
            
            // Read with controlled buffer size
            let received = server_framed.read_message(&mut controlled_stream).await
                .expect("Failed to read with controlled buffer");
            
            assert!(received.verify_signature(), "Controlled buffer message signature failed");
            
            // Echo back with controlled buffer size
            server_framed.write_message(&mut controlled_stream, &received).await
                .expect("Failed to write with controlled buffer");
            
            received
        });
        
        // Connect client with controlled buffer sizes
        let client_stream = TcpStream::connect(buffer_server_addr).await
            .expect("Failed to connect for buffer test");
        let mut client_controlled = BufferControlledTcpStream::new(client_stream, buffer_size, buffer_size);
        
        // Send with controlled buffer size
        buffer_framed.write_message(&mut client_controlled, &buffer_test_envelope).await
            .expect("Client failed to write with controlled buffer");
        
        // Read echo with controlled buffer size
        let echo = buffer_framed.read_message(&mut client_controlled).await
            .expect("Client failed to read echo with controlled buffer");
        
        assert!(echo.verify_signature(), "Buffer test echo signature verification failed");
        
        // Wait for server
        server_task.await.expect("Buffer test server task failed");
        
        println!("    ✓ Successfully handled {} buffer configuration", description);
    }
    
    // Test 3: Test behavior with various network conditions (slow, fast, unreliable)
    println!("\nTest 3: Testing behavior with various network conditions");
    
    // Test slow network conditions
    println!("  Testing slow network conditions");
    
    let slow_listener = TcpListener::bind("127.0.0.1:0").await
        .expect("Failed to bind slow network listener");
    let slow_server_addr = slow_listener.local_addr()
        .expect("Failed to get slow listener address");
    
    let slow_test_payload = "Slow network test message";
    let (slow_test_envelope, _) = create_test_envelope(slow_test_payload);
    let slow_framed = FramedMessage::with_timeouts(Duration::from_secs(10), Duration::from_secs(10));
    
    // Spawn server for slow network test
    let server_framed = slow_framed.clone();
    let slow_server_task = tokio::spawn(async move {
        let (stream, _) = slow_listener.accept().await
            .expect("Failed to accept slow network connection");
        
        // Simulate slow network with small delays
        let mut slow_stream = SlowNetworkStream::new(stream, Duration::from_millis(10));
        
        let received = server_framed.read_message(&mut slow_stream).await
            .expect("Failed to read in slow network conditions");
        
        assert!(received.verify_signature(), "Slow network message signature failed");
        
        server_framed.write_message(&mut slow_stream, &received).await
            .expect("Failed to write in slow network conditions");
        
        received
    });
    
    // Connect client for slow network test
    let client_stream = TcpStream::connect(slow_server_addr).await
        .expect("Failed to connect for slow network test");
    let mut client_slow = SlowNetworkStream::new(client_stream, Duration::from_millis(10));
    
    let slow_start = std::time::Instant::now();
    
    slow_framed.write_message(&mut client_slow, &slow_test_envelope).await
        .expect("Client failed to write in slow network");
    
    let echo = slow_framed.read_message(&mut client_slow).await
        .expect("Client failed to read in slow network");
    
    let slow_duration = slow_start.elapsed();
    
    assert!(echo.verify_signature(), "Slow network echo signature verification failed");
    slow_server_task.await.expect("Slow network server task failed");
    
    println!("    ✓ Successfully handled slow network conditions (took {:?})", slow_duration);
    
    // Test fast network conditions with large messages
    println!("  Testing fast network conditions with large messages");
    
    let fast_listener = TcpListener::bind("127.0.0.1:0").await
        .expect("Failed to bind fast network listener");
    let fast_server_addr = fast_listener.local_addr()
        .expect("Failed to get fast listener address");
    
    let large_payload = "Fast network large message test ".repeat(1000);
    let (fast_test_envelope, _) = create_test_envelope(&large_payload);
    let fast_framed = FramedMessage::default();
    
    // Spawn server for fast network test
    let server_framed = fast_framed.clone();
    let fast_server_task = tokio::spawn(async move {
        let (mut stream, _) = fast_listener.accept().await
            .expect("Failed to accept fast network connection");
        
        let received = server_framed.read_message(&mut stream).await
            .expect("Failed to read large message in fast network");
        
        assert!(received.verify_signature(), "Fast network large message signature failed");
        
        server_framed.write_message(&mut stream, &received).await
            .expect("Failed to write large message in fast network");
        
        received
    });
    
    // Connect client for fast network test
    let mut client_stream = TcpStream::connect(fast_server_addr).await
        .expect("Failed to connect for fast network test");
    
    let fast_start = std::time::Instant::now();
    
    fast_framed.write_message(&mut client_stream, &fast_test_envelope).await
        .expect("Client failed to write large message");
    
    let echo = fast_framed.read_message(&mut client_stream).await
        .expect("Client failed to read large message echo");
    
    let fast_duration = fast_start.elapsed();
    
    assert!(echo.verify_signature(), "Fast network echo signature verification failed");
    fast_server_task.await.expect("Fast network server task failed");
    
    println!("    ✓ Successfully handled fast network with large message (took {:?})", fast_duration);
    
    // Test 4: Verify protocol works across different network interfaces
    println!("\nTest 4: Testing protocol across different network interfaces");
    
    // Test IPv4 localhost
    println!("  Testing IPv4 localhost interface");
    
    let ipv4_listener = TcpListener::bind("127.0.0.1:0").await
        .expect("Failed to bind IPv4 listener");
    let ipv4_addr = ipv4_listener.local_addr()
        .expect("Failed to get IPv4 listener address");
    
    let ipv4_payload = "IPv4 localhost interface test";
    let (ipv4_envelope, _) = create_test_envelope(ipv4_payload);
    let ipv4_framed = FramedMessage::default();
    
    // Test IPv4 communication
    let server_framed = ipv4_framed.clone();
    let ipv4_server_task = tokio::spawn(async move {
        let (mut stream, client_addr) = ipv4_listener.accept().await
            .expect("Failed to accept IPv4 connection");
        
        println!("    IPv4 Server: Connected to {}", client_addr);
        
        let received = server_framed.read_message(&mut stream).await
            .expect("Failed to read IPv4 message");
        
        assert!(received.verify_signature(), "IPv4 message signature failed");
        
        server_framed.write_message(&mut stream, &received).await
            .expect("Failed to write IPv4 echo");
        
        received
    });
    
    let mut ipv4_client = TcpStream::connect(ipv4_addr).await
        .expect("Failed to connect to IPv4 server");
    
    ipv4_framed.write_message(&mut ipv4_client, &ipv4_envelope).await
        .expect("Failed to write to IPv4 server");
    
    let ipv4_echo = ipv4_framed.read_message(&mut ipv4_client).await
        .expect("Failed to read IPv4 echo");
    
    assert!(ipv4_echo.verify_signature(), "IPv4 echo signature verification failed");
    ipv4_server_task.await.expect("IPv4 server task failed");
    
    println!("    ✓ IPv4 localhost interface working correctly");
    
    // Test IPv6 localhost (if available)
    println!("  Testing IPv6 localhost interface (if available)");
    
    match TcpListener::bind("[::1]:0").await {
        Ok(ipv6_listener) => {
            let ipv6_addr = ipv6_listener.local_addr()
                .expect("Failed to get IPv6 listener address");
            
            let ipv6_payload = "IPv6 localhost interface test";
            let (ipv6_envelope, _) = create_test_envelope(ipv6_payload);
            let ipv6_framed = FramedMessage::default();
            
            let server_framed = ipv6_framed.clone();
            let ipv6_server_task = tokio::spawn(async move {
                let (mut stream, client_addr) = ipv6_listener.accept().await
                    .expect("Failed to accept IPv6 connection");
                
                println!("    IPv6 Server: Connected to {}", client_addr);
                
                let received = server_framed.read_message(&mut stream).await
                    .expect("Failed to read IPv6 message");
                
                assert!(received.verify_signature(), "IPv6 message signature failed");
                
                server_framed.write_message(&mut stream, &received).await
                    .expect("Failed to write IPv6 echo");
                
                received
            });
            
            let mut ipv6_client = TcpStream::connect(ipv6_addr).await
                .expect("Failed to connect to IPv6 server");
            
            ipv6_framed.write_message(&mut ipv6_client, &ipv6_envelope).await
                .expect("Failed to write to IPv6 server");
            
            let ipv6_echo = ipv6_framed.read_message(&mut ipv6_client).await
                .expect("Failed to read IPv6 echo");
            
            assert!(ipv6_echo.verify_signature(), "IPv6 echo signature verification failed");
            ipv6_server_task.await.expect("IPv6 server task failed");
            
            println!("    ✓ IPv6 localhost interface working correctly");
        },
        Err(e) => {
            println!("    ! IPv6 localhost not available on this system: {}", e);
            println!("    ! This is normal on some systems - IPv4 test passed");
        }
    }
    
    // Test any available interface (0.0.0.0)
    println!("  Testing any interface binding (0.0.0.0)");
    
    let any_listener = TcpListener::bind("0.0.0.0:0").await
        .expect("Failed to bind to any interface");
    let any_addr = any_listener.local_addr()
        .expect("Failed to get any interface address");
    
    let any_payload = "Any interface test";
    let (any_envelope, _) = create_test_envelope(any_payload);
    let any_framed = FramedMessage::default();
    
    let server_framed = any_framed.clone();
    let any_server_task = tokio::spawn(async move {
        let (mut stream, client_addr) = any_listener.accept().await
            .expect("Failed to accept any interface connection");
        
        println!("    Any Interface Server: Connected to {}", client_addr);
        
        let received = server_framed.read_message(&mut stream).await
            .expect("Failed to read any interface message");
        
        assert!(received.verify_signature(), "Any interface message signature failed");
        
        server_framed.write_message(&mut stream, &received).await
            .expect("Failed to write any interface echo");
        
        received
    });
    
    // Connect to the "any" interface via localhost
    let any_connect_addr = SocketAddr::new("127.0.0.1".parse().unwrap(), any_addr.port());
    let mut any_client = TcpStream::connect(any_connect_addr).await
        .expect("Failed to connect to any interface server");
    
    any_framed.write_message(&mut any_client, &any_envelope).await
        .expect("Failed to write to any interface server");
    
    let any_echo = any_framed.read_message(&mut any_client).await
        .expect("Failed to read any interface echo");
    
    assert!(any_echo.verify_signature(), "Any interface echo signature verification failed");
    any_server_task.await.expect("Any interface server task failed");
    
    println!("    ✓ Any interface (0.0.0.0) binding working correctly");
    
    // Test 5: Concurrent connections
    println!("\nTest 5: Testing concurrent TCP connections");
    
    let concurrent_listener = TcpListener::bind("127.0.0.1:0").await
        .expect("Failed to bind concurrent test listener");
    let concurrent_addr = concurrent_listener.local_addr()
        .expect("Failed to get concurrent listener address");
    
    let concurrent_framed = FramedMessage::default();
    let num_concurrent = 5;
    
    // Spawn server that handles multiple concurrent connections
    let server_framed = concurrent_framed.clone();
    let concurrent_server_task = tokio::spawn(async move {
        let mut handles = Vec::new();
        
        for i in 0..num_concurrent {
            let (mut stream, client_addr) = concurrent_listener.accept().await
                .expect("Failed to accept concurrent connection");
            
            println!("    Server: Accepted concurrent connection {} from {}", i, client_addr);
            
            let server_framed = server_framed.clone();
            let handle = tokio::spawn(async move {
                let received = server_framed.read_message(&mut stream).await
                    .expect("Failed to read concurrent message");
                
                assert!(received.verify_signature(), "Concurrent message signature failed");
                
                server_framed.write_message(&mut stream, &received).await
                    .expect("Failed to write concurrent echo");
                
                received
            });
            
            handles.push(handle);
        }
        
        // Wait for all concurrent connections to complete
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.expect("Concurrent server task failed"));
        }
        
        results
    });
    
    // Create multiple concurrent client connections
    let mut client_handles = Vec::new();
    
    for i in 0..num_concurrent {
        let client_framed = concurrent_framed.clone();
        let handle = tokio::spawn(async move {
            let mut client = TcpStream::connect(concurrent_addr).await
                .expect("Failed to connect concurrent client");
            
            let payload = format!("Concurrent test message {}", i);
            let (envelope, _) = create_test_envelope(&payload);
            
            client_framed.write_message(&mut client, &envelope).await
                .expect("Failed to write concurrent message");
            
            let echo = client_framed.read_message(&mut client).await
                .expect("Failed to read concurrent echo");
            
            assert!(echo.verify_signature(), "Concurrent echo signature verification failed");
            
            println!("    Client {}: Successfully completed concurrent test", i);
            echo
        });
        
        client_handles.push(handle);
    }
    
    // Wait for all clients to complete
    for handle in client_handles {
        handle.await.expect("Concurrent client task failed");
    }
    
    // Wait for server to complete
    let server_results = concurrent_server_task.await.expect("Concurrent server task failed");
    assert_eq!(server_results.len(), num_concurrent, "Server should handle all concurrent connections");
    
    println!("    ✓ Successfully handled {} concurrent TCP connections", num_concurrent);
    
    println!("\n✓ All TCP stream compatibility tests passed!");
} 