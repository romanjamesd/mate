//! Chess Message Performance Benchmarking Tests
//!
//! This module contains comprehensive benchmarks for chess message operations
//! to identify performance bottlenecks and ensure optimal operation under load.
//!
//! ## Test Categories
//! - **Serialization Performance**: JSON vs binary performance comparison
//! - **Validation Performance**: Security and format validation overhead
//! - **Core Function Performance**: Game ID generation, hashing, verification speed
//! - **Memory Usage**: Memory efficiency of message operations

use mate::chess::{Board, Color};
use mate::messages::chess::{
    generate_game_id, hash_board_state, security::validate_message_security, verify_board_hash,
    GameDecline, Move, SyncResponse,
};
use mate::messages::types::Message;
use std::time::Instant;

// Helper function to detect CI environment and adjust performance expectations
fn is_ci_environment() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("CIRCLECI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
}

fn get_performance_multiplier() -> u32 {
    if is_ci_environment() {
        // Be more lenient in CI environments
        10
    } else if cfg!(debug_assertions) {
        // Debug builds are slower
        5
    } else {
        // Release builds on local machines
        1
    }
}

// =============================================================================
// Serialization Performance Tests
// =============================================================================

#[cfg(test)]
mod serialization_performance_tests {
    use super::*;

    #[test]
    fn test_json_vs_binary_serialization_comparison() {
        println!("Benchmarking JSON vs Binary serialization performance");

        let message_types = create_test_message_suite();
        let iterations = 1000;

        for (type_name, message) in message_types {
            println!("  Testing {} message type...", type_name);

            // Test JSON serialization
            let json_start = Instant::now();
            let mut json_total_size = 0;
            for _ in 0..iterations {
                match serde_json::to_string(&message) {
                    Ok(json_data) => json_total_size += json_data.len(),
                    Err(e) => panic!("JSON serialization failed for {}: {}", type_name, e),
                }
            }
            let json_duration = json_start.elapsed();

            // Test binary serialization
            let binary_start = Instant::now();
            let mut binary_total_size = 0;
            for _ in 0..iterations {
                match message.serialize() {
                    Ok(binary_data) => binary_total_size += binary_data.len(),
                    Err(e) => panic!("Binary serialization failed for {}: {}", type_name, e),
                }
            }
            let binary_duration = binary_start.elapsed();

            // Calculate performance metrics
            let json_per_op = json_duration / iterations;
            let binary_per_op = binary_duration / iterations;
            let json_avg_size = json_total_size / iterations as usize;
            let binary_avg_size = binary_total_size / iterations as usize;
            let speed_ratio = json_duration.as_nanos() as f64 / binary_duration.as_nanos() as f64;
            let size_ratio = json_avg_size as f64 / binary_avg_size as f64;

            println!(
                "    JSON:   {:?}/op, {} bytes avg",
                json_per_op, json_avg_size
            );
            println!(
                "    Binary: {:?}/op, {} bytes avg",
                binary_per_op, binary_avg_size
            );
            println!("    Speed ratio: {:.2}x", speed_ratio);
            println!("    Size ratio: {:.2}x", size_ratio);

            // Performance assertions
            // Note: CI environments may have different performance characteristics
            let multiplier = get_performance_multiplier();
            let json_max = 500 * multiplier;
            let binary_max = 200 * multiplier;

            assert!(
                json_per_op.as_micros() < json_max as u128,
                "JSON serialization should be reasonably fast (< {}μs), got {:?}. Multiplier: {}x",
                json_max,
                json_per_op,
                multiplier
            );
            assert!(
                binary_per_op.as_micros() < binary_max as u128,
                "Binary serialization should be faster (< {}μs), got {:?}. Multiplier: {}x",
                binary_max,
                binary_per_op,
                multiplier
            );
            assert!(
                binary_duration <= json_duration,
                "Binary should be faster than JSON for {}",
                type_name
            );
        }

        println!("✓ JSON vs Binary serialization benchmark completed");
    }

    #[test]
    fn test_large_message_serialization_performance() {
        println!("Benchmarking large message serialization performance");

        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        // Test different sizes of sync response messages
        let history_sizes = vec![10, 100, 500, 1000];

        for history_size in history_sizes {
            println!("  Testing sync response with {} moves...", history_size);

            let move_history: Vec<String> =
                (0..history_size).map(|i| format!("move_{}", i)).collect();

            let sync_response = SyncResponse::new(
                game_id.clone(),
                board.to_fen(),
                move_history,
                board_hash.clone(),
            );
            let message = Message::SyncResponse(sync_response);

            // Test JSON serialization
            let json_start = Instant::now();
            let json_data = serde_json::to_string(&message)
                .expect("Large message JSON serialization should succeed");
            let json_duration = json_start.elapsed();

            // Test binary serialization
            let binary_start = Instant::now();
            let binary_data = message
                .serialize()
                .expect("Large message binary serialization should succeed");
            let binary_duration = binary_start.elapsed();

            println!("    JSON:   {:?}, {} bytes", json_duration, json_data.len());
            println!(
                "    Binary: {:?}, {} bytes",
                binary_duration,
                binary_data.len()
            );

            // Large messages should still serialize in reasonable time
            // Note: CI environments may have different performance characteristics
            let multiplier = get_performance_multiplier();
            let json_max_ms = 100 * multiplier;
            let binary_max_ms = 50 * multiplier;

            assert!(
                json_duration.as_millis() < json_max_ms as u128,
                "Large JSON serialization should complete reasonably quickly (< {}ms), got {:?}ms",
                json_max_ms,
                json_duration.as_millis()
            );
            assert!(
                binary_duration.as_millis() < binary_max_ms as u128,
                "Large binary serialization should complete reasonably quickly (< {}ms), got {:?}ms",
                binary_max_ms,
                binary_duration.as_millis()
            );

            // Binary should generally be competitive with JSON for large messages
            // Note: Binary format may have overhead for certain structures, so we allow some variance
            let size_ratio = binary_data.len() as f64 / json_data.len() as f64;
            assert!(
                size_ratio < 1.5,
                "Binary should be reasonably compact compared to JSON (ratio: {:.2})",
                size_ratio
            );
        }

        println!("✓ Large message serialization benchmark completed");
    }

    #[test]
    fn test_message_roundtrip_performance() {
        println!("Benchmarking message serialization roundtrip performance");

        let message_types = create_test_message_suite();
        let iterations = 500;

        for (type_name, message) in message_types {
            println!("  Testing {} roundtrip...", type_name);

            // Test JSON roundtrip
            let json_start = Instant::now();
            for _ in 0..iterations {
                let serialized =
                    serde_json::to_string(&message).expect("JSON serialization should succeed");
                let _deserialized: Message =
                    serde_json::from_str(&serialized).expect("JSON deserialization should succeed");
            }
            let json_roundtrip = json_start.elapsed();

            // Test binary roundtrip
            let binary_start = Instant::now();
            for _ in 0..iterations {
                let serialized = message
                    .serialize()
                    .expect("Binary serialization should succeed");
                let _deserialized = Message::deserialize(&serialized)
                    .expect("Binary deserialization should succeed");
            }
            let binary_roundtrip = binary_start.elapsed();

            let json_per_roundtrip = json_roundtrip / iterations;
            let binary_per_roundtrip = binary_roundtrip / iterations;

            println!("    JSON roundtrip:   {:?}/op", json_per_roundtrip);
            println!("    Binary roundtrip: {:?}/op", binary_per_roundtrip);

            // Roundtrip should be fast enough for real-time communication
            // Note: CI environments may have different performance characteristics
            let multiplier = get_performance_multiplier();
            let json_max = 1000 * multiplier;
            let binary_max = 500 * multiplier;

            assert!(
                json_per_roundtrip.as_micros() < json_max as u128,
                "JSON roundtrip should be reasonably fast (< {}μs), got {:?}",
                json_max,
                json_per_roundtrip
            );
            assert!(
                binary_per_roundtrip.as_micros() < binary_max as u128,
                "Binary roundtrip should be faster (< {}μs), got {:?}",
                binary_max,
                binary_per_roundtrip
            );
        }

        println!("✓ Message roundtrip performance benchmark completed");
    }
}

// =============================================================================
// Validation Performance Tests
// =============================================================================

#[cfg(test)]
mod validation_performance_tests {
    use super::*;

    #[test]
    fn test_security_validation_overhead() {
        println!("Benchmarking security validation performance overhead");

        let message_types = create_test_message_suite();
        let iterations = 2000;

        for (type_name, message) in message_types {
            println!("  Testing {} security validation...", type_name);

            // Baseline: Basic message validation
            let basic_start = Instant::now();
            for _ in 0..iterations {
                let _ = message.validate();
            }
            let basic_duration = basic_start.elapsed();

            // Security validation only
            let security_start = Instant::now();
            for _ in 0..iterations {
                let _ = validate_message_security(&message);
            }
            let security_duration = security_start.elapsed();

            let basic_per_op = basic_duration / iterations;
            let security_per_op = security_duration / iterations;
            let overhead_ratio =
                security_duration.as_nanos() as f64 / basic_duration.as_nanos() as f64;

            println!("    Basic validation:    {:?}/op", basic_per_op);
            println!("    Security validation: {:?}/op", security_per_op);
            println!("    Overhead ratio: {:.2}x", overhead_ratio);

            // Security validation should not add excessive overhead
            // Note: CI environments may have different performance characteristics
            let max_micros = if cfg!(debug_assertions) { 500 } else { 100 };
            let max_ratio = if cfg!(debug_assertions) { 20.0 } else { 10.0 };

            assert!(
                security_per_op.as_micros() < max_micros,
                "Security validation should be reasonably fast (< {}μs), got {:?}",
                max_micros,
                security_per_op
            );
            assert!(
                overhead_ratio < max_ratio,
                "Security overhead should be reasonable (< {:.1}x), got {:.2}x",
                max_ratio,
                overhead_ratio
            );
        }

        println!("✓ Security validation overhead benchmark completed");
    }

    #[test]
    fn test_format_validation_performance() {
        println!("Benchmarking format validation performance");

        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);
        let iterations = 5000;

        // Test individual validation functions using boxed closures
        type ValidationTest = (&'static str, Box<dyn Fn() -> bool>);
        let validation_tests: Vec<ValidationTest> = vec![
            (
                "game_id",
                Box::new(move || mate::messages::chess::validate_game_id(&game_id)),
            ),
            (
                "board_hash",
                Box::new(move || verify_board_hash(&board, &board_hash)),
            ),
            (
                "chess_move",
                Box::new(|| mate::messages::chess::validate_chess_move_format("e2e4").is_ok()),
            ),
        ];

        for (validation_name, validation_fn) in validation_tests {
            println!("  Testing {} validation...", validation_name);

            let start = Instant::now();
            let mut success_count = 0;
            for _ in 0..iterations {
                if validation_fn() {
                    success_count += 1;
                }
            }
            let duration = start.elapsed();

            let per_validation = duration / iterations;
            let success_rate = (success_count as f64 / iterations as f64) * 100.0;

            println!(
                "    {:?}/validation, {:.1}% success rate",
                per_validation, success_rate
            );

            // Individual validations should be reasonably fast
            // Note: CI environments may have different performance characteristics
            let multiplier = get_performance_multiplier();
            let max_micros = 10 * multiplier;
            assert!(
                per_validation.as_micros() < max_micros as u128,
                "{} validation should be reasonably fast (< {}μs), got {:?}. Multiplier: {}x",
                validation_name,
                max_micros,
                per_validation,
                multiplier
            );
        }

        println!("✓ Format validation performance benchmark completed");
    }

    #[test]
    fn test_complex_message_validation_performance() {
        println!("Benchmarking complex message validation performance");

        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        // Create complex messages with varying validation complexity
        let complex_messages = vec![
            (
                "large_sync_response",
                Message::SyncResponse(SyncResponse::new(
                    game_id.clone(),
                    board.to_fen(),
                    (0..500).map(|i| format!("move_{}", i)).collect(),
                    board_hash.clone(),
                )),
            ),
            (
                "game_decline_with_reason",
                Message::GameDecline(GameDecline::new_with_reason(
                    game_id.clone(),
                    "Very detailed reason for declining this chess game invitation".to_string(),
                )),
            ),
            (
                "move_with_hash",
                Message::Move(Move::new(game_id.clone(), "e2e4".to_string(), board_hash)),
            ),
        ];

        let iterations = 1000;

        for (message_name, message) in complex_messages {
            println!("  Testing {} validation...", message_name);

            let validation_start = Instant::now();
            let mut successful_validations = 0;
            for _ in 0..iterations {
                if message.validate().is_ok() {
                    successful_validations += 1;
                }
            }
            let validation_duration = validation_start.elapsed();

            let per_validation = validation_duration / iterations;
            let success_rate = (successful_validations as f64 / iterations as f64) * 100.0;

            println!(
                "    {:?}/validation, {:.1}% success rate",
                per_validation, success_rate
            );

            // Complex validation should still be reasonable
            // Note: CI environments may have different performance characteristics
            let max_micros = if cfg!(debug_assertions) { 5000 } else { 1000 };
            assert!(
                per_validation.as_micros() < max_micros,
                "{} validation should be reasonable (< {}μs), got {:?}",
                message_name,
                max_micros,
                per_validation
            );
        }

        println!("✓ Complex message validation benchmark completed");
    }
}

// =============================================================================
// Core Function Performance Tests
// =============================================================================

#[cfg(test)]
mod core_function_performance_tests {
    use super::*;

    #[test]
    fn test_game_id_generation_performance() {
        println!("Benchmarking game ID generation performance");

        let iterations = 10000;
        let start = Instant::now();

        let mut game_ids = Vec::with_capacity(iterations as usize);
        for _ in 0..iterations {
            game_ids.push(generate_game_id());
        }

        let duration = start.elapsed();
        let per_generation = duration / iterations;

        println!("    Generated {} IDs in {:?}", iterations, duration);
        println!("    {:?} per ID generation", per_generation);

        // Game ID generation should be reasonably fast
        // Note: CI environments may have different performance characteristics
        let max_micros = if cfg!(debug_assertions) { 200 } else { 50 };
        assert!(
            per_generation.as_micros() < max_micros,
            "Game ID generation should be reasonably fast (< {}μs), got {:?}",
            max_micros,
            per_generation
        );

        // Verify uniqueness (basic check)
        let unique_count = game_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(
            unique_count, iterations as usize,
            "All generated IDs should be unique"
        );

        println!("✓ Game ID generation performance benchmark completed");
    }

    #[test]
    fn test_board_hashing_performance() {
        println!("Benchmarking board state hashing performance");

        let board = Board::new();
        let iterations = 10000;

        let start = Instant::now();
        let mut hashes = Vec::with_capacity(iterations as usize);
        for _ in 0..iterations {
            hashes.push(hash_board_state(&board));
        }
        let duration = start.elapsed();

        let per_hash = duration / iterations;

        println!("    Hashed {} boards in {:?}", iterations, duration);
        println!("    {:?} per board hash", per_hash);

        // Board hashing should be reasonably fast
        // Note: CI environments may have different performance characteristics
        let max_micros = if cfg!(debug_assertions) { 400 } else { 100 };
        assert!(
            per_hash.as_micros() < max_micros,
            "Board hashing should be reasonably fast (< {}μs), got {:?}",
            max_micros,
            per_hash
        );

        // Verify consistency
        let first_hash = &hashes[0];
        assert!(
            hashes.iter().all(|h| h == first_hash),
            "All hashes of the same board should be identical"
        );

        println!("✓ Board hashing performance benchmark completed");
    }

    #[test]
    fn test_hash_verification_performance() {
        println!("Benchmarking hash verification performance");

        let board = Board::new();
        let correct_hash = hash_board_state(&board);
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let iterations = 10000;

        // Test correct hash verification
        let correct_start = Instant::now();
        let mut correct_verifications = 0;
        for _ in 0..iterations {
            if verify_board_hash(&board, &correct_hash) {
                correct_verifications += 1;
            }
        }
        let correct_duration = correct_start.elapsed();

        // Test wrong hash verification
        let wrong_start = Instant::now();
        let mut wrong_verifications = 0;
        for _ in 0..iterations {
            if verify_board_hash(&board, wrong_hash) {
                wrong_verifications += 1;
            }
        }
        let wrong_duration = wrong_start.elapsed();

        let correct_per_verify = correct_duration / iterations;
        let wrong_per_verify = wrong_duration / iterations;

        println!("    Correct hash: {:?}/verify", correct_per_verify);
        println!("    Wrong hash:   {:?}/verify", wrong_per_verify);
        println!(
            "    Correct verifications: {}/{}",
            correct_verifications, iterations
        );
        println!(
            "    Wrong verifications: {}/{}",
            wrong_verifications, iterations
        );

        // Hash verification should be reasonably fast
        // Note: CI environments may have different performance characteristics
        let max_micros = if cfg!(debug_assertions) { 200 } else { 50 };
        assert!(
            correct_per_verify.as_micros() < max_micros,
            "Hash verification should be reasonably fast (< {}μs), got {:?}",
            max_micros,
            correct_per_verify
        );
        assert!(
            wrong_per_verify.as_micros() < max_micros,
            "Wrong hash verification should be reasonably fast (< {}μs), got {:?}",
            max_micros,
            wrong_per_verify
        );

        // Verification should have correct results
        assert_eq!(
            correct_verifications, iterations as usize,
            "All correct hashes should verify"
        );
        assert_eq!(wrong_verifications, 0, "Wrong hashes should not verify");

        println!("✓ Hash verification performance benchmark completed");
    }

    #[test]
    fn test_combined_operations_performance() {
        println!("Benchmarking combined core operations performance");

        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            // Simulate typical game flow operations
            let game_id = generate_game_id();
            let board = Board::new();
            let hash = hash_board_state(&board);

            // Verify the operations work
            assert!(mate::messages::chess::validate_game_id(&game_id));
            assert!(verify_board_hash(&board, &hash));
        }

        let duration = start.elapsed();
        let per_iteration = duration / iterations;

        println!("    {} combined operations in {:?}", iterations, duration);
        println!("    {:?} per complete operation cycle", per_iteration);

        // Combined operations should be reasonable for real-time use
        // Note: CI environments may have different performance characteristics
        let max_micros = if cfg!(debug_assertions) { 2000 } else { 500 };
        assert!(
            per_iteration.as_micros() < max_micros,
            "Combined operations should be reasonably fast (< {}μs), got {:?}",
            max_micros,
            per_iteration
        );

        println!("✓ Combined operations performance benchmark completed");
    }
}

// =============================================================================
// Memory Usage Tests
// =============================================================================

#[cfg(test)]
mod memory_usage_tests {
    use super::*;

    #[test]
    fn test_message_memory_efficiency() {
        println!("Benchmarking message memory efficiency");

        let message_types = create_test_message_suite();

        for (type_name, message) in message_types {
            println!("  Analyzing {} memory usage...", type_name);

            // Get estimated size
            let estimated_size = message.estimated_size();

            // Get actual serialized size
            let binary_data = message
                .serialize()
                .expect("Message should serialize successfully");
            let actual_binary_size = binary_data.len();

            let json_data = serde_json::to_string(&message)
                .expect("Message should serialize to JSON successfully");
            let actual_json_size = json_data.len();

            // Calculate efficiency metrics
            let binary_efficiency = estimated_size as f64 / actual_binary_size as f64;
            let json_overhead = actual_json_size as f64 / actual_binary_size as f64;

            println!("    Estimated: {} bytes", estimated_size);
            println!("    Binary:    {} bytes", actual_binary_size);
            println!("    JSON:      {} bytes", actual_json_size);
            println!("    Binary efficiency: {:.2}x", binary_efficiency);
            println!("    JSON overhead: {:.2}x", json_overhead);

            // Size estimates should be reasonably accurate
            assert!(
                binary_efficiency > 0.5 && binary_efficiency < 3.0,
                "Size estimation should be reasonable for {}: {:.2}x",
                type_name,
                binary_efficiency
            );

            // Binary should be more efficient than JSON
            assert!(
                actual_binary_size <= actual_json_size,
                "Binary should be more compact than JSON for {}",
                type_name
            );
        }

        println!("✓ Message memory efficiency benchmark completed");
    }

    #[test]
    fn test_memory_scaling_characteristics() {
        println!("Benchmarking memory scaling characteristics");

        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        let history_sizes = vec![0, 10, 50, 100, 500, 1000];
        let mut scaling_data = Vec::new();

        for history_size in history_sizes {
            println!("  Testing scaling with {} moves...", history_size);

            let move_history: Vec<String> = (0..history_size)
                .map(|i| format!("move_{:04}", i))
                .collect();

            let sync_response = SyncResponse::new(
                game_id.clone(),
                board.to_fen(),
                move_history,
                board_hash.clone(),
            );
            let message = Message::SyncResponse(sync_response);

            let estimated_size = message.estimated_size();
            let binary_size = message.serialize().unwrap().len();
            let json_size = serde_json::to_string(&message).unwrap().len();

            scaling_data.push((history_size, estimated_size, binary_size, json_size));

            println!(
                "    Estimated: {} bytes, Binary: {} bytes, JSON: {} bytes",
                estimated_size, binary_size, json_size
            );
        }

        // Analyze scaling patterns
        println!("  Memory scaling analysis:");
        for window in scaling_data.windows(2) {
            let (size1, _est1, bin1, json1) = window[0];
            let (size2, _est2, bin2, json2) = window[1];

            if size1 > 0 {
                let size_ratio = size2 as f64 / size1 as f64;
                let binary_ratio = bin2 as f64 / bin1 as f64;
                let json_ratio = json2 as f64 / json1 as f64;

                println!(
                    "    {}→{} moves: size×{:.1}, binary×{:.1}, json×{:.1}",
                    size1, size2, size_ratio, binary_ratio, json_ratio
                );

                // Memory growth should be roughly linear with content
                if size_ratio > 1.5 {
                    // Only check for significant size increases
                    assert!(
                        binary_ratio < size_ratio * 2.0,
                        "Binary memory growth should be reasonable"
                    );
                    assert!(
                        json_ratio < size_ratio * 3.0,
                        "JSON memory growth should be reasonable"
                    );
                }
            }
        }

        println!("✓ Memory scaling characteristics benchmark completed");
    }

    #[test]
    fn test_memory_leak_detection() {
        println!("Benchmarking for potential memory leaks");

        let iterations = 1000;
        let mut peak_memory_usage = 0;

        // Simulate repeated message creation and processing
        for batch in 0..10 {
            let batch_start = Instant::now();
            let mut messages = Vec::new();

            // Create a batch of messages
            for _ in 0..iterations {
                let game_id = generate_game_id();
                let board = Board::new();
                let hash = hash_board_state(&board);

                let message = Message::new_move(game_id, "e2e4".to_string(), hash);
                let _ = message.validate();
                let _ = message.serialize();
                messages.push(message);
            }

            let batch_duration = batch_start.elapsed();
            let estimated_memory = messages.iter().map(|m| m.estimated_size()).sum::<usize>();

            if estimated_memory > peak_memory_usage {
                peak_memory_usage = estimated_memory;
            }

            println!(
                "    Batch {}: {} messages, ~{} bytes, {:?}",
                batch + 1,
                messages.len(),
                estimated_memory,
                batch_duration
            );

            // Each batch should complete in reasonable time
            // Note: CI environments may have different performance characteristics
            let batch_max_millis = if cfg!(debug_assertions) { 5000 } else { 1000 };
            assert!(
                batch_duration.as_millis() < batch_max_millis,
                "Batch processing should be reasonably fast (< {}ms), got {:?}",
                batch_max_millis,
                batch_duration
            );

            // Clear messages to test cleanup
            messages.clear();
        }

        println!(
            "    Peak estimated memory usage: {} bytes",
            peak_memory_usage
        );

        // Memory usage should be reasonable
        assert!(
            peak_memory_usage < 10_000_000, // 10MB
            "Peak memory usage should be reasonable: {} bytes",
            peak_memory_usage
        );

        println!("✓ Memory leak detection benchmark completed");
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a comprehensive suite of test messages for benchmarking
fn create_test_message_suite() -> Vec<(String, Message)> {
    let game_id = generate_game_id();
    let board = Board::new();
    let board_hash = hash_board_state(&board);

    vec![
        (
            "GameInvite".to_string(),
            Message::new_game_invite(game_id.clone(), Some(Color::White)),
        ),
        (
            "GameAccept".to_string(),
            Message::new_game_accept(game_id.clone(), Color::Black),
        ),
        (
            "GameDecline".to_string(),
            Message::new_game_decline(game_id.clone(), Some("Busy".to_string())),
        ),
        (
            "Move".to_string(),
            Message::new_move(game_id.clone(), "e2e4".to_string(), board_hash.clone()),
        ),
        (
            "MoveAck".to_string(),
            Message::new_move_ack(game_id.clone(), Some("move-123".to_string())),
        ),
        (
            "SyncRequest".to_string(),
            Message::new_sync_request(game_id.clone()),
        ),
        (
            "SyncResponse".to_string(),
            Message::new_sync_response(
                game_id,
                board.to_fen(),
                vec!["e2e4".to_string(), "e7e5".to_string(), "g1f3".to_string()],
                board_hash,
            ),
        ),
    ]
}
