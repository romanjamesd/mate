use mate::messages::types::Message;

#[test]
fn test_message_serialization_integration() {
    // Test various message scenarios
    let test_cases = vec![
        Message::new_ping(0, "".to_string()),
        Message::new_ping(1, "simple".to_string()),
        Message::new_ping(u64::MAX, "max nonce".to_string()),
        Message::new_pong(42, "response".to_string()),
        Message::new_pong(999, "long response with special chars: !@#$%^&*()".to_string()),
    ];

    for (i, original) in test_cases.iter().enumerate() {
        // Serialize
        let bytes = original.serialize()
            .unwrap_or_else(|e| panic!("Test case {}: Failed to serialize: {}", i, e));
        
        // Verify non-empty
        assert!(!bytes.is_empty(), "Test case {}: Serialized bytes should not be empty", i);
        
        // Deserialize
        let restored = Message::deserialize(&bytes)
            .unwrap_or_else(|e| panic!("Test case {}: Failed to deserialize: {}", i, e));
        
        // Verify equality
        assert_eq!(original.get_nonce(), restored.get_nonce(), "Test case {}: Nonce mismatch", i);
        assert_eq!(original.get_payload(), restored.get_payload(), "Test case {}: Payload mismatch", i);
        assert_eq!(original.message_type(), restored.message_type(), "Test case {}: Type mismatch", i);
    }
}

#[test]
fn test_serialization_size_efficiency() {
    let small_msg = Message::new_ping(0, "".to_string());
    let medium_msg = Message::new_ping(42, "medium payload".to_string());
    
    let small_bytes = small_msg.serialize().expect("Failed to serialize small message");
    let medium_bytes = medium_msg.serialize().expect("Failed to serialize medium message");
    
    // Verify that serialization is reasonably efficient
    assert!(small_bytes.len() < 50, "Small message should serialize to < 50 bytes");
    assert!(medium_bytes.len() < 100, "Medium message should serialize to < 100 bytes");
    assert!(medium_bytes.len() > small_bytes.len(), "Medium message should be larger than small");
}

#[test]
fn test_cross_message_type_verification() {
    // Create messages with identical content but different types
    let nonce = 12345u64;
    let payload = "identical payload content";
    
    let ping = Message::new_ping(nonce, payload.to_string());
    let pong = Message::new_pong(nonce, payload.to_string());
    
    // Serialize both
    let ping_bytes = ping.serialize().expect("Failed to serialize ping");
    let pong_bytes = pong.serialize().expect("Failed to serialize pong");
    
    // Verify they produce different serialized data
    assert_ne!(ping_bytes, pong_bytes, "Messages with same content but different types should serialize differently");
    
    // Verify each can be deserialized correctly
    let ping_restored = Message::deserialize(&ping_bytes).expect("Failed to deserialize ping");
    let pong_restored = Message::deserialize(&pong_bytes).expect("Failed to deserialize pong");
    
    assert!(ping_restored.is_ping(), "Restored ping should be ping type");
    assert!(pong_restored.is_pong(), "Restored pong should be pong type");
    assert_eq!(ping_restored.get_nonce(), nonce);
    assert_eq!(pong_restored.get_nonce(), nonce);
    assert_eq!(ping_restored.get_payload(), payload);
    assert_eq!(pong_restored.get_payload(), payload);
}

#[test]
fn test_serialization_with_extreme_values() {
    // Test with extreme nonce values
    let test_cases = vec![
        (0u64, "min nonce"),
        (u64::MAX, "max nonce"),
        (u64::MAX / 2, "mid nonce"),
    ];
    
    for (nonce, description) in test_cases {
        let ping = Message::new_ping(nonce, format!("Testing {}", description));
        let pong = Message::new_pong(nonce, format!("Response for {}", description));
        
        // Test ping
        let ping_bytes = ping.serialize().expect("Failed to serialize ping with extreme nonce");
        let ping_restored = Message::deserialize(&ping_bytes).expect("Failed to deserialize ping with extreme nonce");
        assert_eq!(ping.get_nonce(), ping_restored.get_nonce(), "Nonce mismatch for {}", description);
        
        // Test pong
        let pong_bytes = pong.serialize().expect("Failed to serialize pong with extreme nonce");
        let pong_restored = Message::deserialize(&pong_bytes).expect("Failed to deserialize pong with extreme nonce");
        assert_eq!(pong.get_nonce(), pong_restored.get_nonce(), "Nonce mismatch for {}", description);
    }
}

#[test]
fn test_serialization_consistency_across_calls() {
    // Test that multiple serialization calls produce identical results
    let message = Message::new_ping(42, "consistency test".to_string());
    
    let mut serialized_results = Vec::new();
    
    // Serialize the same message multiple times
    for _ in 0..10 {
        let bytes = message.serialize().expect("Serialization should not fail");
        serialized_results.push(bytes);
    }
    
    // Verify all results are identical
    let first_result = &serialized_results[0];
    for (i, result) in serialized_results.iter().enumerate() {
        assert_eq!(first_result, result, "Serialization result {} differs from first result", i);
    }
}

#[test]
fn test_realistic_message_flow() {
    // Simulate a realistic ping-pong exchange
    let ping_nonce = 12345u64;
    let ping_payload = "Hello, peer! This is a ping message.";
    
    // Create and serialize ping
    let ping = Message::new_ping(ping_nonce, ping_payload.to_string());
    let ping_bytes = ping.serialize().expect("Failed to serialize ping");
    
    // Simulate network transmission by deserializing
    let received_ping = Message::deserialize(&ping_bytes).expect("Failed to deserialize received ping");
    
    // Verify received ping
    assert!(received_ping.is_ping(), "Received message should be ping");
    assert_eq!(received_ping.get_nonce(), ping_nonce);
    assert_eq!(received_ping.get_payload(), ping_payload);
    
    // Create response pong with same nonce
    let pong_payload = "Hello back! This is a pong response.";
    let pong = Message::new_pong(received_ping.get_nonce(), pong_payload.to_string());
    let pong_bytes = pong.serialize().expect("Failed to serialize pong");
    
    // Simulate network transmission of response
    let received_pong = Message::deserialize(&pong_bytes).expect("Failed to deserialize received pong");
    
    // Verify received pong
    assert!(received_pong.is_pong(), "Received message should be pong");
    assert_eq!(received_pong.get_nonce(), ping_nonce, "Pong should have same nonce as ping");
    assert_eq!(received_pong.get_payload(), pong_payload);
}

#[test]
fn test_serialization_performance_characteristics() {
    // Test serialization performance with various payload sizes
    let payload_sizes = vec![0, 10, 100, 1000, 10000];
    
    for size in payload_sizes {
        let payload = "x".repeat(size);
        let message = Message::new_ping(42, payload.clone());
        
        // Measure serialization
        let start = std::time::Instant::now();
        let bytes = message.serialize().expect("Serialization should not fail");
        let serialize_duration = start.elapsed();
        
        // Measure deserialization
        let start = std::time::Instant::now();
        let restored = Message::deserialize(&bytes).expect("Deserialization should not fail");
        let deserialize_duration = start.elapsed();
        
        // Verify correctness
        assert_eq!(message.get_payload(), restored.get_payload());
        
        // Basic performance assertions (should be very fast for reasonable sizes)
        assert!(serialize_duration.as_millis() < 100, "Serialization took too long for {} byte payload", size);
        assert!(deserialize_duration.as_millis() < 100, "Deserialization took too long for {} byte payload", size);
        
        // Verify serialized size is reasonable (should be close to payload size + overhead)
        if size > 0 {
            assert!(bytes.len() >= size, "Serialized size should be at least payload size");
            assert!(bytes.len() < size + 100, "Serialized size should not have excessive overhead");
        }
    }
} 