//! Helper functions for creating test data
//!
//! This module provides utilities for creating test messages, envelopes,
//! and other test data structures used across multiple test files.

use crate::common::mock_streams::MockStream;
use mate::crypto::Identity;
use mate::messages::wire::FramedMessage;
use mate::messages::{Message, SignedEnvelope};

/// Create a test SignedEnvelope with a known message
pub fn create_test_envelope(payload: &str) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Create a test SignedEnvelope with a unique identifier
pub fn create_test_envelope_with_nonce(payload: &str, nonce: u64) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(nonce, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Helper function to write multiple messages to a single buffer in sequence
pub async fn write_multiple_messages_to_buffer(messages: &[(SignedEnvelope, Message)]) -> Vec<u8> {
    let framed_message = FramedMessage::default();
    let mut stream = MockStream::new();

    for (envelope, _) in messages {
        framed_message
            .write_message(&mut stream, envelope)
            .await
            .expect("Failed to write message to buffer");
    }

    stream.get_written_data().to_vec()
}
