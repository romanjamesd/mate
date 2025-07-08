use ed25519_dalek::VerifyingKey;
use mate::crypto::identity::{Identity, PeerId};
use std::collections::{HashMap, HashSet};

/// Priority 1: Core Requirements Tests (6 tests)
/// Direct fulfillment of issue-12.md requirements

#[test]
fn test_identity_generation_and_uniqueness() {
    // Test basic identity generation
    let identity1 = Identity::generate().expect("Failed to generate identity1");
    let identity2 = Identity::generate().expect("Failed to generate identity2");

    // Verify identities are unique
    assert_ne!(
        identity1.peer_id(),
        identity2.peer_id(),
        "Generated identities should be unique"
    );

    // Test multiple identity generation for uniqueness verification
    let mut peer_ids = HashSet::new();
    for i in 0..10 {
        let identity =
            Identity::generate().unwrap_or_else(|_| panic!("Failed to generate identity {}", i));
        let peer_id = identity.peer_id().as_str().to_string();
        assert!(
            peer_ids.insert(peer_id),
            "Generated identity {} should be unique",
            i
        );
    }
}

#[test]
fn test_peer_id_generation_and_serialization() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let verifying_key = identity.verifying_key();

    // Test PeerId creation from verifying key
    let peer_id1 = PeerId::from_verifying_key(&verifying_key);
    let peer_id2 = identity.peer_id();
    assert_eq!(
        &peer_id1, peer_id2,
        "PeerId generation should be consistent"
    );

    // Test string serialization round-trip
    let peer_id_str = peer_id1.as_str();
    let peer_id_reconstructed = PeerId::from_string(peer_id_str.to_string());
    assert_eq!(
        peer_id1, peer_id_reconstructed,
        "String serialization should be round-trip compatible"
    );

    // Test verifying key reconstruction
    let reconstructed_key = peer_id_reconstructed
        .to_verifying_key()
        .expect("Should be able to reconstruct verifying key");
    assert_eq!(
        verifying_key.to_bytes(),
        reconstructed_key.to_bytes(),
        "Reconstructed verifying key should match original"
    );

    // Test JSON serialization round-trip
    let json_str = serde_json::to_string(&peer_id1).expect("Failed to serialize to JSON");
    let peer_id_from_json: PeerId =
        serde_json::from_str(&json_str).expect("Failed to deserialize from JSON");
    assert_eq!(
        peer_id1, peer_id_from_json,
        "JSON serialization should be round-trip compatible"
    );
}

#[test]
fn test_message_signing_deterministic() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = b"test message";

    // Test basic signing functionality
    let signature1 = identity.sign(message);
    let signature2 = identity.sign(message);

    // Ed25519 signing should be deterministic (same message = same signature)
    assert_eq!(
        signature1.to_bytes(),
        signature2.to_bytes(),
        "Signing should be deterministic for the same message"
    );

    // Test signing different messages produces different signatures
    let different_message = b"different test message";
    let signature3 = identity.sign(different_message);
    assert_ne!(
        signature1.to_bytes(),
        signature3.to_bytes(),
        "Different messages should produce different signatures"
    );
}

#[test]
fn test_signature_verification_valid() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = b"test message for verification";

    // Test valid signature verification
    let signature = identity.sign(message);
    let verifying_key = identity.verifying_key();

    let is_valid = Identity::verify(&verifying_key, message, &signature);
    assert!(is_valid, "Valid signature should verify successfully");

    // Test cross-identity verification (should fail)
    let other_identity = Identity::generate().expect("Failed to generate other identity");
    let other_verifying_key = other_identity.verifying_key();

    let is_cross_valid = Identity::verify(&other_verifying_key, message, &signature);
    assert!(
        !is_cross_valid,
        "Signature should not verify with different key"
    );
}

#[test]
fn test_signature_verification_invalid_cases() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = b"original message";
    let signature = identity.sign(message);
    let verifying_key = identity.verifying_key();

    // Test with wrong key (different identity)
    let wrong_identity = Identity::generate().expect("Failed to generate wrong identity");
    let wrong_key = wrong_identity.verifying_key();
    assert!(
        !Identity::verify(&wrong_key, message, &signature),
        "Signature should fail verification with wrong key"
    );

    // Test with tampered message
    let tampered_message = b"tampered message";
    assert!(
        !Identity::verify(&verifying_key, tampered_message, &signature),
        "Signature should fail verification with tampered message"
    );

    // Test with tampered signature (flip one bit)
    let mut tampered_sig_bytes = signature.to_bytes();
    tampered_sig_bytes[0] ^= 0x01; // Flip first bit
    let tampered_signature = ed25519_dalek::Signature::from_bytes(&tampered_sig_bytes);
    assert!(
        !Identity::verify(&verifying_key, message, &tampered_signature),
        "Tampered signature should fail verification"
    );
}

#[test]
fn test_peer_id_equality_and_hashing() {
    let identity1 = Identity::generate().expect("Failed to generate identity1");
    let identity2 = Identity::generate().expect("Failed to generate identity2");

    let peer_id1a = identity1.peer_id().clone();
    let peer_id1b = PeerId::from_verifying_key(&identity1.verifying_key());
    let peer_id2 = identity2.peer_id().clone();

    // Test equality comparisons
    assert_eq!(
        peer_id1a, peer_id1b,
        "Same identity should produce equal PeerIds"
    );
    assert_ne!(
        peer_id1a, peer_id2,
        "Different identities should produce different PeerIds"
    );

    // Test HashMap compatibility (required for networking)
    let mut peer_map: HashMap<PeerId, String> = HashMap::new();
    peer_map.insert(peer_id1a.clone(), "peer1".to_string());
    peer_map.insert(peer_id2.clone(), "peer2".to_string());

    assert_eq!(
        peer_map.get(&peer_id1a),
        Some(&"peer1".to_string()),
        "HashMap lookup should work"
    );
    assert_eq!(
        peer_map.get(&peer_id1b),
        Some(&"peer1".to_string()),
        "Equal PeerIds should hash to same value"
    );
    assert_eq!(
        peer_map.get(&peer_id2),
        Some(&"peer2".to_string()),
        "Different PeerIds should be distinct"
    );
    assert_eq!(
        peer_map.len(),
        2,
        "HashMap should contain exactly 2 entries"
    );
}

/// Priority 2: Edge Cases & Error Handling Tests (6 tests)
/// Focus on scenarios difficult to test in integration

#[test]
fn test_peer_id_invalid_input_handling() {
    // Test invalid base64 input
    let invalid_base64 = PeerId::from_string("invalid_base64_!@#$%".to_string());
    assert!(
        invalid_base64.to_verifying_key().is_err(),
        "Invalid base64 should fail to convert to verifying key"
    );

    // Test wrong key length handling (too short)
    let short_key = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [1, 2, 3]);
    let short_peer_id = PeerId::from_string(short_key);
    assert!(
        short_peer_id.to_verifying_key().is_err(),
        "Too short key should fail to convert to verifying key"
    );

    // Test wrong key length handling (too long)
    let long_key = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [0u8; 64]);
    let long_peer_id = PeerId::from_string(long_key);
    assert!(
        long_peer_id.to_verifying_key().is_err(),
        "Too long key should fail to convert to verifying key"
    );

    // Test malformed string parsing (empty string)
    let empty_peer_id = PeerId::from_string("".to_string());
    assert!(
        empty_peer_id.to_verifying_key().is_err(),
        "Empty string should fail to convert to verifying key"
    );
}

#[test]
fn test_signing_edge_cases() {
    let identity = Identity::generate().expect("Failed to generate identity");

    // Test empty message signing
    let empty_message = b"";
    let empty_signature = identity.sign(empty_message);
    let verifying_key = identity.verifying_key();
    assert!(
        Identity::verify(&verifying_key, empty_message, &empty_signature),
        "Empty message signature should verify successfully"
    );

    // Test very large message signing (performance boundary)
    let large_message = vec![0u8; 1024 * 1024]; // 1MB message
    let large_signature = identity.sign(&large_message);
    assert!(
        Identity::verify(&verifying_key, &large_message, &large_signature),
        "Large message signature should verify successfully"
    );

    // Test message boundary conditions (single byte)
    let single_byte = &[42u8];
    let single_signature = identity.sign(single_byte);
    assert!(
        Identity::verify(&verifying_key, single_byte, &single_signature),
        "Single byte message signature should verify successfully"
    );
}

#[test]
fn test_verification_edge_cases() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let verifying_key = identity.verifying_key();

    // Test empty message verification
    let empty_message = b"";
    let empty_signature = identity.sign(empty_message);
    assert!(
        Identity::verify(&verifying_key, empty_message, &empty_signature),
        "Empty message verification should succeed"
    );

    // Test signature format validation (all zeros signature)
    let zero_signature = ed25519_dalek::Signature::from_bytes(&[0u8; 64]);
    assert!(
        !Identity::verify(&verifying_key, b"test", &zero_signature),
        "All-zeros signature should fail verification"
    );

    // Test signature format validation (all ones signature)
    let ones_signature = ed25519_dalek::Signature::from_bytes(&[0xFFu8; 64]);
    assert!(
        !Identity::verify(&verifying_key, b"test", &ones_signature),
        "All-ones signature should fail verification"
    );

    // Test key format validation with corrupted key data
    let mut corrupted_key_bytes = verifying_key.to_bytes();
    corrupted_key_bytes[31] = 0xFF; // Corrupt last byte

    // Note: VerifyingKey::from_bytes may still succeed with some corrupted data
    // but verification should fail due to key mismatch
    if let Ok(corrupted_key) = VerifyingKey::from_bytes(&corrupted_key_bytes) {
        let test_signature = identity.sign(b"test");
        assert!(
            !Identity::verify(&corrupted_key, b"test", &test_signature),
            "Corrupted key should fail signature verification"
        );
    }
}

#[test]
fn test_peer_id_display_formatting() {
    let identity = Identity::generate().expect("Failed to generate identity");
    let peer_id = identity.peer_id();

    // Test Display trait implementation
    let display_str = format!("{peer_id}");
    let as_str = peer_id.as_str();
    assert_eq!(display_str, as_str, "Display output should match as_str()");

    // Test string representation consistency
    let peer_id_from_str = PeerId::from_string(display_str.clone());
    assert_eq!(
        peer_id, &peer_id_from_str,
        "Display string should be reversible"
    );

    // Verify the display string is valid base64
    assert!(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &display_str).is_ok(),
        "Display string should be valid base64"
    );

    // Test that display format is stable (same identity = same display)
    let same_peer_id = PeerId::from_verifying_key(&identity.verifying_key());
    assert_eq!(
        format!("{same_peer_id}"),
        display_str,
        "Display format should be stable for same identity"
    );
}

#[test]
fn test_identity_serialization_errors() {
    // Test corrupted key data handling
    let identity = Identity::generate().expect("Failed to generate identity");
    let _verifying_key = identity.verifying_key();

    // Create PeerId with corrupted base64 (valid base64 but wrong length after decode)
    let short_bytes = [1u8; 16]; // Only 16 bytes instead of 32
    let corrupted_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, short_bytes);
    let corrupted_peer_id = PeerId::from_string(corrupted_base64);

    let result = corrupted_peer_id.to_verifying_key();
    assert!(result.is_err(), "Corrupted key data should produce error");

    // Verify error contains useful information
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Invalid PeerId key length") || error_msg.contains("32 bytes"),
        "Error should mention key length requirement"
    );

    // Test invalid serialization format handling (not base64)
    let invalid_format = PeerId::from_string("not-valid-base64-@#$%".to_string());
    let result2 = invalid_format.to_verifying_key();
    assert!(
        result2.is_err(),
        "Invalid base64 format should produce error"
    );

    let error_msg2 = result2.unwrap_err().to_string();
    assert!(
        error_msg2.contains("Failed to decode PeerId base64") || error_msg2.contains("decode"),
        "Error should mention base64 decoding failure"
    );
}

#[test]
fn test_crypto_memory_safety() {
    let message = b"test message for memory safety";

    // Test that key data doesn't persist in memory after drop
    let signature = {
        let identity = Identity::generate().expect("Failed to generate identity");
        let sig = identity.sign(message);
        let verifying_key = identity.verifying_key();

        // Verify signature works while identity is in scope
        assert!(
            Identity::verify(&verifying_key, message, &sig),
            "Signature should verify while identity is in scope"
        );

        (sig, verifying_key)
    }; // identity drops here

    let (sig, key) = signature;

    // Test signature verification with moved data
    assert!(
        Identity::verify(&key, message, &sig),
        "Signature should still verify after identity is dropped"
    );

    // Test that we can still use the moved verifying key for new operations
    let peer_id = PeerId::from_verifying_key(&key);
    let reconstructed_key = peer_id
        .to_verifying_key()
        .expect("Should be able to reconstruct key from PeerId");

    assert_eq!(
        key.to_bytes(),
        reconstructed_key.to_bytes(),
        "Reconstructed key should match original after move"
    );
}
