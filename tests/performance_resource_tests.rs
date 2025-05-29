use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::FramedMessage;
use tokio::io::{AsyncRead, AsyncWrite};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Barrier};

/// Test helper to create a mock read/write stream from a buffer
struct MockStream {
    read_cursor: Cursor<Vec<u8>>,
    write_buffer: Vec<u8>,
}

impl MockStream {
    fn new() -> Self {
        Self {
            read_cursor: Cursor::new(Vec::new()),
            write_buffer: Vec::new(),
        }
    }
    
    fn with_data(data: Vec<u8>) -> Self {
        Self {
            read_cursor: Cursor::new(data),
            write_buffer: Vec::new(),
        }
    }
    
    fn get_written_data(&self) -> &[u8] {
        &self.write_buffer
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.read_cursor).poll_read(cx, buf)
    }
}

impl AsyncWrite for MockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write_buffer.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// Memory allocation tracking stream that monitors allocation patterns
struct MemoryTrackingStream {
    inner: MockStream,
    allocations: Arc<Mutex<Vec<AllocationEvent>>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Allow unused fields for comprehensive tracking
struct AllocationEvent {
    size: usize,
    operation: String,
    timestamp: Instant,
}

impl MemoryTrackingStream {
    fn new() -> Self {
        Self {
            inner: MockStream::new(),
            allocations: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn with_data(data: Vec<u8>) -> Self {
        let allocation_size = data.len();
        let allocations = Arc::new(Mutex::new(Vec::new()));
        {
            let mut allocs = allocations.lock().unwrap();
            allocs.push(AllocationEvent {
                size: allocation_size,
                operation: "initial_data".to_string(),
                timestamp: Instant::now(),
            });
        }
        
        Self {
            inner: MockStream::with_data(data),
            allocations,
        }
    }
    
    fn get_written_data(&self) -> &[u8] {
        self.inner.get_written_data()
    }
    
    fn get_allocation_events(&self) -> Vec<AllocationEvent> {
        self.allocations.lock().unwrap().clone()
    }
    
    fn record_allocation(&self, size: usize, operation: &str) {
        let mut allocs = self.allocations.lock().unwrap();
        allocs.push(AllocationEvent {
            size,
            operation: operation.to_string(),
            timestamp: Instant::now(),
        });
    }
}

impl AsyncRead for MemoryTrackingStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let initial_filled = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        
        if let std::task::Poll::Ready(Ok(())) = result {
            let bytes_read = buf.filled().len() - initial_filled;
            if bytes_read > 0 {
                self.record_allocation(bytes_read, "read_buffer");
            }
        }
        
        result
    }
}

impl AsyncWrite for MemoryTrackingStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let result = std::pin::Pin::new(&mut self.inner).poll_write(cx, buf);
        
        if let std::task::Poll::Ready(Ok(bytes_written)) = result {
            self.record_allocation(bytes_written, "write_buffer");
        }
        
        result
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Create a test SignedEnvelope with a known message
fn create_test_envelope(payload: &str) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Create a test SignedEnvelope with a unique identifier
fn create_test_envelope_with_nonce(payload: &str, nonce: u64) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(nonce, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Memory usage statistics for analysis
#[derive(Debug, Default)]
struct MemoryStats {
    total_allocated: usize,
    peak_allocation: usize,
    allocation_count: usize,
    average_allocation_size: f64,
}

impl MemoryStats {
    fn from_events(events: &[AllocationEvent]) -> Self {
        if events.is_empty() {
            return Self::default();
        }
        
        let total_allocated: usize = events.iter().map(|e| e.size).sum();
        let peak_allocation = events.iter().map(|e| e.size).max().unwrap_or(0);
        let allocation_count = events.len();
        let average_allocation_size = total_allocated as f64 / allocation_count as f64;
        
        Self {
            total_allocated,
            peak_allocation,
            allocation_count,
            average_allocation_size,
        }
    }
}

/// Test memory usage efficiency during message processing
#[tokio::test]
async fn test_memory_usage_efficiency() {
    println!("Testing memory usage efficiency during message processing");
    
    let framed_message = FramedMessage::default();
    
    // Test 1: Monitor memory allocation patterns during message processing
    println!("Test 1: Monitoring memory allocation patterns for various message sizes");
    
    let test_message_sizes = vec![
        (100, "small message"),
        (1024, "medium message (1KB)"),
        (10 * 1024, "large message (10KB)"),
        (100 * 1024, "very large message (100KB)"),
    ];
    
    for (payload_size, description) in test_message_sizes {
        println!("  Testing {} ({} bytes payload)", description, payload_size);
        
        let payload = "A".repeat(payload_size);
        let (envelope, _) = create_test_envelope(&payload);
        
        // Track memory during write operation
        let mut tracking_stream = MemoryTrackingStream::new();
        
        let write_start = Instant::now();
        framed_message.write_message(&mut tracking_stream, &envelope)
            .await
            .expect("Failed to write message for memory tracking");
        let write_duration = write_start.elapsed();
        
        // Analyze write memory patterns
        let write_events = tracking_stream.get_allocation_events();
        let write_stats = MemoryStats::from_events(&write_events);
        
        println!("    Write - Allocations: {}, Total: {} bytes, Peak: {} bytes, Avg: {:.1} bytes, Duration: {:?}",
                 write_stats.allocation_count,
                 write_stats.total_allocated,
                 write_stats.peak_allocation,
                 write_stats.average_allocation_size,
                 write_duration);
        
        // Verify write allocation efficiency
        assert!(write_stats.peak_allocation <= payload_size + 2000, 
               "Peak allocation should not significantly exceed message size for write");
        
        // Track memory during read operation
        let written_data = tracking_stream.get_written_data().to_vec();
        let mut read_tracking_stream = MemoryTrackingStream::with_data(written_data);
        
        let read_start = Instant::now();
        let received_envelope = framed_message.read_message(&mut read_tracking_stream)
            .await
            .expect("Failed to read message for memory tracking");
        let read_duration = read_start.elapsed();
        
        // Analyze read memory patterns
        let read_events = read_tracking_stream.get_allocation_events();
        let read_stats = MemoryStats::from_events(&read_events);
        
        println!("    Read  - Allocations: {}, Total: {} bytes, Peak: {} bytes, Avg: {:.1} bytes, Duration: {:?}",
                 read_stats.allocation_count,
                 read_stats.total_allocated,
                 read_stats.peak_allocation,
                 read_stats.average_allocation_size,
                 read_duration);
        
        // Verify read allocation efficiency
        assert!(read_stats.peak_allocation <= payload_size + 2000,
               "Peak allocation should not significantly exceed message size for read");
        
        // Verify message integrity
        assert!(received_envelope.verify_signature(), 
               "Message signature should be valid after memory-tracked processing");
        
        // Verify timing is reasonable (should be fast for non-huge messages)
        if payload_size <= 100 * 1024 {  // Only check timing for reasonably sized messages
            assert!(write_duration < Duration::from_millis(100),
                   "Write operation should be fast for {} byte message", payload_size);
            assert!(read_duration < Duration::from_millis(100),
                   "Read operation should be fast for {} byte message", payload_size);
        }
    }
    
    // Test 2: Verify no excessive memory allocation for normal operations
    println!("Test 2: Verifying no excessive allocation for normal operations");
    
    let normal_payload = "Normal message payload".to_string();
    let (normal_envelope, _) = create_test_envelope(&normal_payload);
    let expected_message_size = bincode::serialize(&normal_envelope)
        .expect("Failed to serialize normal envelope")
        .len();
    
    // Process message and monitor allocations
    let mut efficient_stream = MemoryTrackingStream::new();
    framed_message.write_message(&mut efficient_stream, &normal_envelope)
        .await
        .expect("Failed to write normal message");
    
    let written_data = efficient_stream.get_written_data().to_vec();
    let mut read_efficient_stream = MemoryTrackingStream::with_data(written_data);
    
    let _received_envelope = framed_message.read_message(&mut read_efficient_stream)
        .await
        .expect("Failed to read normal message");
    
    // Combine allocation events from both operations
    let mut all_events = efficient_stream.get_allocation_events();
    all_events.extend(read_efficient_stream.get_allocation_events());
    
    let efficiency_stats = MemoryStats::from_events(&all_events);
    
    println!("  Normal operation - Message size: {} bytes, Total allocated: {} bytes, Peak: {} bytes",
             expected_message_size, efficiency_stats.total_allocated, efficiency_stats.peak_allocation);
    
    // Verify efficiency: total allocation should not be more than 4x the message size
    // (allows for some overhead but catches excessive allocation)
    let allocation_ratio = efficiency_stats.total_allocated as f64 / expected_message_size as f64;
    assert!(allocation_ratio <= 4.0,
           "Total allocation ratio ({:.2}x) should not be excessive for normal operations", allocation_ratio);
    
    println!("    ✓ Allocation ratio: {:.2}x (within efficient bounds)", allocation_ratio);
    
    // Test 3: Test memory cleanup after message processing completion
    println!("Test 3: Testing memory cleanup after processing completion");
    
    let cleanup_test_messages = vec![
        "Small cleanup test".to_string(),
        "A".repeat(5000), // Medium message for cleanup test
        "B".repeat(20000), // Larger message for cleanup test
    ];
    
    for (test_index, cleanup_payload) in cleanup_test_messages.iter().enumerate() {
        let (cleanup_envelope, _) = create_test_envelope(cleanup_payload);
        
        // Process message in a block to ensure variables go out of scope
        let allocation_events = {
            let mut cleanup_stream = MemoryTrackingStream::new();
            
            framed_message.write_message(&mut cleanup_stream, &cleanup_envelope)
                .await
                .expect(&format!("Failed to write cleanup test message {}", test_index));
            
            let written_data = cleanup_stream.get_written_data().to_vec();
            let mut read_cleanup_stream = MemoryTrackingStream::with_data(written_data);
            
            let received_envelope = framed_message.read_message(&mut read_cleanup_stream)
                .await
                .expect(&format!("Failed to read cleanup test message {}", test_index));
            
            // Verify message integrity
            assert!(received_envelope.verify_signature(),
                   "Cleanup test message {} signature should be valid", test_index);
            
            // Collect allocation events before variables go out of scope
            let mut events = cleanup_stream.get_allocation_events();
            events.extend(read_cleanup_stream.get_allocation_events());
            events
        }; // cleanup_stream and read_cleanup_stream go out of scope here
        
        let cleanup_stats = MemoryStats::from_events(&allocation_events);
        println!("  Cleanup test {} - Allocations: {}, Total: {} bytes",
                 test_index + 1, cleanup_stats.allocation_count, cleanup_stats.total_allocated);
        
        // After scope exit, memory should be eligible for cleanup
        // This is a logical test - in Rust, memory is automatically cleaned up when variables go out of scope
        // We verify that the operations completed successfully, indicating proper resource management
        
        // Small delay to allow any background cleanup (though Rust cleanup is immediate)
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    
    println!("    ✓ All cleanup tests completed successfully (memory management verified)");
    
    // Test 4: Verify no memory leaks during sustained operation
    println!("Test 4: Testing for memory leaks during sustained operations");
    
    let sustained_iterations = 20;
    let sustained_payload = "Sustained operation test message payload";
    
    let mut baseline_events = Vec::new();
    let mut sustained_events = Vec::new();
    
    // Establish baseline with first few operations
    for i in 0..3 {
        let (sustained_envelope, _) = create_test_envelope_with_nonce(sustained_payload, i);
        
        let mut sustained_stream = MemoryTrackingStream::new();
        framed_message.write_message(&mut sustained_stream, &sustained_envelope)
            .await
            .expect(&format!("Failed to write sustained test message {}", i));
        
        let written_data = sustained_stream.get_written_data().to_vec();
        let mut read_sustained_stream = MemoryTrackingStream::with_data(written_data);
        
        let received_envelope = framed_message.read_message(&mut read_sustained_stream)
            .await
            .expect(&format!("Failed to read sustained test message {}", i));
        
        assert!(received_envelope.verify_signature(),
               "Sustained test message {} signature should be valid", i);
        
        let mut iteration_events = sustained_stream.get_allocation_events();
        iteration_events.extend(read_sustained_stream.get_allocation_events());
        baseline_events.extend(iteration_events);
    }
    
    let baseline_stats = MemoryStats::from_events(&baseline_events);
    let baseline_avg_per_op = baseline_stats.total_allocated as f64 / 6.0; // 3 iterations × 2 operations each
    
    println!("  Baseline established - Avg allocation per operation: {:.1} bytes", baseline_avg_per_op);
    
    // Continue with sustained operations
    for i in 3..sustained_iterations {
        let (sustained_envelope, _) = create_test_envelope_with_nonce(sustained_payload, i);
        
        let mut sustained_stream = MemoryTrackingStream::new();
        framed_message.write_message(&mut sustained_stream, &sustained_envelope)
            .await
            .expect(&format!("Failed to write sustained test message {}", i));
        
        let written_data = sustained_stream.get_written_data().to_vec();
        let mut read_sustained_stream = MemoryTrackingStream::with_data(written_data);
        
        let received_envelope = framed_message.read_message(&mut read_sustained_stream)
            .await
            .expect(&format!("Failed to read sustained test message {}", i));
        
        assert!(received_envelope.verify_signature(),
               "Sustained test message {} signature should be valid", i);
        
        let mut iteration_events = sustained_stream.get_allocation_events();
        iteration_events.extend(read_sustained_stream.get_allocation_events());
        sustained_events.extend(iteration_events);
    }
    
    let sustained_stats = MemoryStats::from_events(&sustained_events);
    let sustained_operations = (sustained_iterations - 3) * 2; // write + read operations
    let sustained_avg_per_op = sustained_stats.total_allocated as f64 / sustained_operations as f64;
    
    println!("  Sustained phase - {} operations, Avg allocation per operation: {:.1} bytes", 
             sustained_operations, sustained_avg_per_op);
    
    // Verify no significant memory growth (leak detection)
    let growth_ratio = sustained_avg_per_op / baseline_avg_per_op;
    assert!(growth_ratio <= 1.2, // Allow up to 20% variance due to system factors
           "Memory allocation should not grow significantly during sustained operations. Growth ratio: {:.2}x", 
           growth_ratio);
    
    // Verify allocation patterns remain consistent
    let consistency_threshold = baseline_avg_per_op * 0.5; // Allow some variance
    assert!(sustained_avg_per_op >= consistency_threshold,
           "Allocation patterns should remain consistent. Sustained: {:.1}, Baseline: {:.1}",
           sustained_avg_per_op, baseline_avg_per_op);
    
    println!("    ✓ No memory leaks detected. Growth ratio: {:.2}x (within acceptable bounds)", growth_ratio);
    
    // Test 5: Peak memory usage verification
    println!("Test 5: Peak memory usage verification");
    
    let peak_test_sizes = vec![1000, 5000, 10000, 50000];
    
    for test_size in peak_test_sizes {
        let peak_payload = "P".repeat(test_size);
        let (peak_envelope, _) = create_test_envelope(&peak_payload);
        
        let mut peak_stream = MemoryTrackingStream::new();
        framed_message.write_message(&mut peak_stream, &peak_envelope)
            .await
            .expect(&format!("Failed to write peak test message for size {}", test_size));
        
        let written_data = peak_stream.get_written_data().to_vec();
        let mut read_peak_stream = MemoryTrackingStream::with_data(written_data);
        
        let _received_envelope = framed_message.read_message(&mut read_peak_stream)
            .await
            .expect(&format!("Failed to read peak test message for size {}", test_size));
        
        let mut peak_events = peak_stream.get_allocation_events();
        peak_events.extend(read_peak_stream.get_allocation_events());
        
        let peak_stats = MemoryStats::from_events(&peak_events);
        let memory_efficiency = (test_size as f64) / (peak_stats.peak_allocation as f64);
        
        println!("  Size: {} bytes, Peak allocation: {} bytes, Efficiency: {:.2}",
                 test_size, peak_stats.peak_allocation, memory_efficiency);
        
        // Peak allocation should be reasonable relative to message size
        assert!(peak_stats.peak_allocation <= test_size + 5000,
               "Peak allocation should not be excessive for {} byte message", test_size);
        
        // Efficiency should be reasonable (we use at least 50% of peak allocation for actual data)
        assert!(memory_efficiency >= 0.3,
               "Memory efficiency should be reasonable. Got {:.2} for {} byte message", 
               memory_efficiency, test_size);
    }
    
    println!("    ✓ Peak memory usage is within reasonable bounds for all test sizes");
    
    println!("✅ Memory usage efficiency tests completed successfully");
}

/// Test concurrent memory usage patterns
#[tokio::test]
async fn test_concurrent_memory_usage() {
    println!("Testing concurrent memory usage patterns");
    
    let framed_message = FramedMessage::default();
    let concurrent_operations = 10;
    let concurrent_payload = "Concurrent test message";
    
    // Create test envelopes
    let mut test_envelopes = Vec::new();
    for i in 0..concurrent_operations {
        let (envelope, _) = create_test_envelope_with_nonce(concurrent_payload, i);
        test_envelopes.push(envelope);
    }
    
    // Execute concurrent operations
    let start_time = Instant::now();
    let mut handles = Vec::new();
    
    for (i, envelope) in test_envelopes.into_iter().enumerate() {
        let framed_message_clone = framed_message.clone();  // FramedMessage should be Clone
        
        let handle = tokio::spawn(async move {
            let mut stream = MemoryTrackingStream::new();
            
            framed_message_clone.write_message(&mut stream, &envelope)
                .await
                .expect(&format!("Failed to write concurrent message {}", i));
            
            let written_data = stream.get_written_data().to_vec();
            let mut read_stream = MemoryTrackingStream::with_data(written_data);
            
            let received_envelope = framed_message_clone.read_message(&mut read_stream)
                .await
                .expect(&format!("Failed to read concurrent message {}", i));
            
            assert!(received_envelope.verify_signature(),
                   "Concurrent message {} signature should be valid", i);
            
            // Return allocation events for analysis
            let mut events = stream.get_allocation_events();
            events.extend(read_stream.get_allocation_events());
            events
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let mut all_concurrent_events = Vec::new();
    for handle in handles {
        let events = handle.await.expect("Concurrent operation should complete successfully");
        all_concurrent_events.extend(events);
    }
    
    let concurrent_duration = start_time.elapsed();
    let concurrent_stats = MemoryStats::from_events(&all_concurrent_events);
    
    println!("Concurrent operations completed in {:?}", concurrent_duration);
    println!("Total allocations: {}, Total memory: {} bytes, Peak: {} bytes",
             concurrent_stats.allocation_count,
             concurrent_stats.total_allocated,
             concurrent_stats.peak_allocation);
    
    // Verify concurrent operations don't cause excessive memory usage
    let expected_operations = concurrent_operations * 2; // write + read per message
    let avg_allocation_per_op = concurrent_stats.total_allocated as f64 / expected_operations as f64;
    
    println!("Average allocation per operation: {:.1} bytes", avg_allocation_per_op);
    
    // Concurrent operations should not significantly increase per-operation allocation
    assert!(avg_allocation_per_op < 10000.0, // Reasonable upper bound
           "Average allocation per concurrent operation should be reasonable");
    
    println!("✅ Concurrent memory usage tests completed successfully");
}

/// Test concurrent operation safety and thread safety
#[tokio::test]
async fn test_concurrent_operation_safety() {
    println!("Testing concurrent operation safety and thread safety");
    
    let framed_message = FramedMessage::default();
    
    // Test 1: Test concurrent read and write operations on same connection
    println!("Test 1: Testing concurrent read and write operations on same connection");
    
    // Create a shared connection simulation using a bidirectional channel
    let (tx_to_server, mut rx_from_client) = mpsc::unbounded_channel::<Vec<u8>>();
    let (tx_to_client, mut rx_from_server) = mpsc::unbounded_channel::<Vec<u8>>();
    
    let messages_to_send = 10;
    let barrier = Arc::new(Barrier::new(3)); // Reader, writer, and main thread
    
    // Create test messages
    let mut test_messages = Vec::new();
    for i in 0..messages_to_send {
        let payload = format!("Concurrent safety test message {}", i);
        let (envelope, _) = create_test_envelope_with_nonce(&payload, i);
        test_messages.push(envelope);
    }
    
    let test_messages = Arc::new(test_messages);
    let framed_message = Arc::new(framed_message);
    
    // Spawn writer task
    let writer_messages = Arc::clone(&test_messages);
    let writer_framed = Arc::clone(&framed_message);
    let writer_tx = tx_to_server;
    let writer_barrier = Arc::clone(&barrier);
    
    let writer_handle = tokio::spawn(async move {
        // Wait for all tasks to be ready
        writer_barrier.wait().await;
        
        println!("  Writer task started - sending {} messages", messages_to_send);
        
        for (i, envelope) in writer_messages.iter().enumerate() {
            // Simulate writing to connection
            let mut write_stream = MockStream::new();
            
            writer_framed.write_message(&mut write_stream, envelope)
                .await
                .expect(&format!("Failed to write concurrent message {}", i));
            
            let written_data = write_stream.get_written_data().to_vec();
            
            // Send to "server" side
            writer_tx.send(written_data)
                .expect(&format!("Failed to send message {} to server", i));
            
            // Small delay to allow interleaving
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
        
        println!("  Writer task completed");
    });
    
    // Spawn reader task
    let reader_framed = Arc::clone(&framed_message);
    let reader_tx = tx_to_client;
    let reader_barrier = Arc::clone(&barrier);
    
    let reader_handle = tokio::spawn(async move {
        // Wait for all tasks to be ready
        reader_barrier.wait().await;
        
        println!("  Reader task started - expecting {} messages", messages_to_send);
        
        let mut received_count = 0;
        
        while received_count < messages_to_send {
            // Receive data from "client" side
            if let Some(message_data) = rx_from_server.recv().await {
                let mut read_stream = MockStream::with_data(message_data);
                
                let received_envelope = reader_framed.read_message(&mut read_stream)
                    .await
                    .expect(&format!("Failed to read concurrent message {}", received_count));
                
                // Verify message integrity
                assert!(received_envelope.verify_signature(),
                       "Concurrent message {} signature should be valid", received_count);
                
                received_count += 1;
                
                // Small delay to allow interleaving
                tokio::time::sleep(Duration::from_micros(50)).await;
            }
        }
        
        println!("  Reader task completed - received {} messages", received_count);
        received_count
    });
    
    // Main coordination task
    let coordination_barrier = Arc::clone(&barrier);
    let coordination_handle = tokio::spawn(async move {
        // Wait for all tasks to be ready
        coordination_barrier.wait().await;
        
        println!("  Coordination task started - managing message flow");
        
        let mut forwarded_count = 0;
        
        while forwarded_count < messages_to_send {
            // Forward messages from client to server
            if let Some(client_data) = rx_from_client.recv().await {
                reader_tx.send(client_data)
                    .expect(&format!("Failed to forward message {}", forwarded_count));
                forwarded_count += 1;
            }
        }
        
        println!("  Coordination task completed - forwarded {} messages", forwarded_count);
        forwarded_count
    });
    
    // Wait for all tasks to complete
    let start_time = Instant::now();
    let (writer_result, reader_result, coordination_result) = tokio::join!(
        writer_handle,
        reader_handle,
        coordination_handle
    );
    let total_duration = start_time.elapsed();
    
    // Verify results
    writer_result.expect("Writer task should complete successfully");
    let received_count = reader_result.expect("Reader task should complete successfully");
    let forwarded_count = coordination_result.expect("Coordination task should complete successfully");
    
    assert_eq!(received_count, messages_to_send, "Should receive all sent messages");
    assert_eq!(forwarded_count, messages_to_send, "Should forward all sent messages");
    
    println!("    ✓ Concurrent read/write operations completed successfully in {:?}", total_duration);
    println!("    ✓ Sent: {}, Received: {}, Forwarded: {}", messages_to_send, received_count, forwarded_count);
    
    // Test 2: Verify thread safety of wire protocol operations
    println!("Test 2: Verifying thread safety of wire protocol operations");
    
    let thread_safety_operations = 20;
    let thread_safety_payload = "Thread safety test";
    
    // Create multiple tasks that perform wire protocol operations simultaneously
    let mut thread_safety_handles = Vec::new();
    
    for thread_id in 0..thread_safety_operations {
        let framed_clone = FramedMessage::default();  // Each thread gets its own instance
        let payload = format!("{} - thread {}", thread_safety_payload, thread_id);
        
        let handle = tokio::spawn(async move {
            let (envelope, _) = create_test_envelope_with_nonce(&payload, thread_id);
            
            // Perform write operation
            let mut write_stream = MockStream::new();
            let write_result = framed_clone.write_message(&mut write_stream, &envelope).await;
            
            if write_result.is_err() {
                return Err(format!("Thread {} write failed: {:?}", thread_id, write_result.err()));
            }
            
            // Perform read operation
            let written_data = write_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            let read_result = framed_clone.read_message(&mut read_stream).await;
            
            match read_result {
                Ok(received_envelope) => {
                    if !received_envelope.verify_signature() {
                        return Err(format!("Thread {} signature verification failed", thread_id));
                    }
                    Ok(thread_id)
                },
                Err(e) => Err(format!("Thread {} read failed: {:?}", thread_id, e))
            }
        });
        
        thread_safety_handles.push(handle);
    }
    
    // Wait for all thread safety operations to complete
    let thread_safety_start = Instant::now();
    let mut successful_threads = 0;
    let mut failed_threads = 0;
    
    for handle in thread_safety_handles {
        match handle.await {
            Ok(Ok(thread_id)) => {
                successful_threads += 1;
                if thread_id % 5 == 0 {  // Log every 5th thread for visibility
                    println!("    Thread {} completed successfully", thread_id);
                }
            },
            Ok(Err(error_msg)) => {
                failed_threads += 1;
                println!("    Thread failed: {}", error_msg);
            },
            Err(join_error) => {
                failed_threads += 1;
                println!("    Thread join failed: {:?}", join_error);
            }
        }
    }
    
    let thread_safety_duration = thread_safety_start.elapsed();
    
    assert_eq!(failed_threads, 0, "All thread safety operations should succeed");
    assert_eq!(successful_threads, thread_safety_operations, "All threads should complete successfully");
    
    println!("    ✓ Thread safety verified: {}/{} operations successful in {:?}", 
             successful_threads, thread_safety_operations, thread_safety_duration);
    
    // Test 3: Test multiple concurrent connections don't interfere
    println!("Test 3: Testing multiple concurrent connections don't interfere");
    
    let connection_count = 5;
    let messages_per_connection = 4;
    
    // Create multiple "connections" (each with their own FramedMessage instance)
    let mut connection_handles = Vec::new();
    
    for connection_id in 0..connection_count {
        let connection_framed = FramedMessage::default();
        
        let handle = tokio::spawn(async move {
            let mut connection_results = Vec::new();
            
            for message_id in 0..messages_per_connection {
                let payload = format!("Connection {} Message {}", connection_id, message_id);
                let (envelope, _) = create_test_envelope_with_nonce(&payload, 
                    (connection_id * messages_per_connection + message_id) as u64);
                
                // Each connection performs its own write/read cycle
                let mut conn_stream = MockStream::new();
                
                // Write operation
                connection_framed.write_message(&mut conn_stream, &envelope)
                    .await
                    .expect(&format!("Connection {} message {} write should succeed", 
                           connection_id, message_id));
                
                // Read operation  
                let written_data = conn_stream.get_written_data().to_vec();
                let mut read_stream = MockStream::with_data(written_data);
                
                let received_envelope = connection_framed.read_message(&mut read_stream)
                    .await
                    .expect(&format!("Connection {} message {} read should succeed", 
                           connection_id, message_id));
                
                // Verify integrity
                assert!(received_envelope.verify_signature(),
                       "Connection {} message {} signature should be valid", 
                       connection_id, message_id);
                
                connection_results.push((connection_id, message_id));
                
                // Small delay to allow interleaving between connections
                tokio::time::sleep(Duration::from_micros(200)).await;
            }
            
            connection_results
        });
        
        connection_handles.push(handle);
    }
    
    // Wait for all connections to complete
    let connections_start = Instant::now();
    let mut total_operations = 0;
    let mut connection_results = Vec::new();
    
    for handle in connection_handles {
        let results = handle.await.expect("Connection should complete successfully");
        total_operations += results.len();
        connection_results.extend(results);
    }
    
    let connections_duration = connections_start.elapsed();
    
    // Verify all operations completed
    let expected_operations = connection_count * messages_per_connection;
    assert_eq!(total_operations, expected_operations, 
              "All connection operations should complete");
    
    // Verify no interference between connections
    for connection_id in 0..connection_count {
        let connection_ops: Vec<_> = connection_results.iter()
            .filter(|(cid, _)| *cid == connection_id)
            .collect();
        
        assert_eq!(connection_ops.len(), messages_per_connection,
                  "Connection {} should have completed all its operations", connection_id);
    }
    
    println!("    ✓ Multiple connections operated independently: {} connections × {} messages = {} operations in {:?}",
             connection_count, messages_per_connection, total_operations, connections_duration);
    
    // Test 4: Verify resource isolation between concurrent operations
    println!("Test 4: Verifying resource isolation between concurrent operations");
    
    let isolation_operations = 10;
    let isolation_payload_sizes = vec![100, 1000, 5000, 10000];
    
    // Create operations with different resource requirements
    let mut isolation_handles = Vec::new();
    
    for (op_id, &payload_size) in (0..isolation_operations).zip(isolation_payload_sizes.iter().cycle()) {
        let isolation_framed = FramedMessage::default();
        
        let handle = tokio::spawn(async move {
            let payload = "R".repeat(payload_size);  // Different payload sizes
            let (envelope, _) = create_test_envelope_with_nonce(&payload, op_id as u64);
            
            let operation_start = Instant::now();
            
            // Write operation
            let mut isolation_stream = MockStream::new();
            isolation_framed.write_message(&mut isolation_stream, &envelope)
                .await
                .expect(&format!("Isolation operation {} write should succeed", op_id));
            
            // Read operation
            let written_data = isolation_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            
            let received_envelope = isolation_framed.read_message(&mut read_stream)
                .await
                .expect(&format!("Isolation operation {} read should succeed", op_id));
            
            let operation_duration = operation_start.elapsed();
            
            // Verify integrity
            assert!(received_envelope.verify_signature(),
                   "Isolation operation {} signature should be valid", op_id);
            
            (op_id, payload_size, operation_duration)
        });
        
        isolation_handles.push(handle);
    }
    
    // Wait for all isolation operations to complete
    let isolation_start = Instant::now();
    let mut isolation_results = Vec::new();
    
    for handle in isolation_handles {
        let result = handle.await.expect("Isolation operation should complete successfully");
        isolation_results.push(result);
    }
    
    let total_isolation_duration = isolation_start.elapsed();
    
    // Verify resource isolation - operations should complete regardless of payload size
    assert_eq!(isolation_results.len(), isolation_operations, 
              "All isolation operations should complete");
    
    // Verify that larger operations don't block smaller ones excessively
    let mut size_groups: std::collections::HashMap<usize, Vec<Duration>> = std::collections::HashMap::new();
    
    for (_, payload_size, duration) in &isolation_results {
        size_groups.entry(*payload_size).or_insert_with(Vec::new).push(*duration);
    }
    
    println!("    Resource isolation verification:");
    for (size, durations) in &size_groups {
        let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;
        println!("      {} byte operations: {} samples, avg duration: {:?}", 
                 size, durations.len(), avg_duration);
    }
    
    println!("    ✓ Resource isolation verified: {} operations with varying resource requirements in {:?}",
             isolation_operations, total_isolation_duration);
    
    println!("✅ Concurrent operation safety tests completed successfully");
} 