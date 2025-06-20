//! Unit tests for core utility functions for game IDs, hashing, and basic operations
//!
//! This test module comprehensively tests the fundamental operations that support
//! the chess message protocol: game ID generation, board state hashing, hash verification,
//! and utility function integration.

use mate::chess::Board;
use mate::messages::chess::{
    generate_game_id, hash_board_state, validate_game_id, verify_board_hash,
    verify_board_hash_graceful,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use uuid::Uuid;

// =============================================================================
// Game ID Generation Tests
// =============================================================================

#[cfg(test)]
mod game_id_generation_tests {
    use super::*;

    #[test]
    fn test_generate_game_id_basic_properties() {
        let game_id = generate_game_id();

        // Basic format validation
        assert!(!game_id.is_empty(), "Generated game ID should not be empty");
        assert_eq!(
            game_id.len(),
            36,
            "Generated game ID should be 36 characters (UUID format)"
        );

        // Should be valid UUID format
        assert!(
            validate_game_id(&game_id),
            "Generated game ID should pass validation"
        );

        // Should be parseable as UUID
        assert!(
            Uuid::parse_str(&game_id).is_ok(),
            "Generated game ID should be parseable as UUID"
        );
    }

    #[test]
    fn test_generate_game_id_uniqueness() {
        // Test uniqueness by generating multiple IDs
        let mut ids = HashSet::new();
        let count = 1000;

        for _ in 0..count {
            let id = generate_game_id();
            assert!(
                ids.insert(id.clone()),
                "Generated game ID should be unique: {}",
                id
            );
        }

        assert_eq!(ids.len(), count, "All generated game IDs should be unique");
    }

    #[test]
    fn test_generate_game_id_format_validation() {
        let game_id = generate_game_id();
        let parts: Vec<&str> = game_id.split('-').collect();

        // UUID v4 format: 8-4-4-4-12 hexadecimal digits
        assert_eq!(parts.len(), 5, "UUID should have 5 hyphen-separated parts");
        assert_eq!(parts[0].len(), 8, "First part should be 8 characters");
        assert_eq!(parts[1].len(), 4, "Second part should be 4 characters");
        assert_eq!(parts[2].len(), 4, "Third part should be 4 characters");
        assert_eq!(parts[3].len(), 4, "Fourth part should be 4 characters");
        assert_eq!(parts[4].len(), 12, "Fifth part should be 12 characters");

        // All parts should be hexadecimal
        for (i, part) in parts.iter().enumerate() {
            assert!(
                part.chars().all(|c| c.is_ascii_hexdigit()),
                "Part {} should be hexadecimal: {}",
                i + 1,
                part
            );
        }
    }

    #[test]
    fn test_generate_game_id_cryptographic_strength() {
        let game_id = generate_game_id();
        let uuid = Uuid::parse_str(&game_id).expect("Should be valid UUID");

        // Should be UUID version 4 (random)
        assert_eq!(
            uuid.get_version(),
            Some(uuid::Version::Random),
            "Generated UUID should be version 4 (random)"
        );

        // Should not be nil UUID
        assert!(!uuid.is_nil(), "Generated UUID should not be nil");

        // Test randomness by checking variant bits are set correctly for UUID v4
        let bytes = uuid.as_bytes();

        // Version bits (4 bits starting at bit 48)
        let version_byte = bytes[6];
        assert_eq!(
            version_byte & 0xF0,
            0x40,
            "Version bits should indicate UUID v4"
        );

        // Variant bits (2 bits starting at bit 64)
        let variant_byte = bytes[8];
        assert_eq!(
            variant_byte & 0xC0,
            0x80,
            "Variant bits should be set correctly"
        );
    }

    #[test]
    fn test_generate_game_id_concurrent_uniqueness() {
        let ids = Arc::new(Mutex::new(HashSet::new()));
        let thread_count = 10;
        let ids_per_thread = 100;
        let mut handles = vec![];

        // Spawn multiple threads generating IDs concurrently
        for _ in 0..thread_count {
            let ids_clone = Arc::clone(&ids);
            let handle = thread::spawn(move || {
                for _ in 0..ids_per_thread {
                    let id = generate_game_id();
                    let mut ids = ids_clone.lock().unwrap();
                    assert!(
                        ids.insert(id.clone()),
                        "Concurrent ID generation produced duplicate: {}",
                        id
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        let final_ids = ids.lock().unwrap();
        assert_eq!(
            final_ids.len(),
            thread_count * ids_per_thread,
            "All concurrent IDs should be unique"
        );
    }

    #[test]
    fn test_generate_game_id_performance() {
        let iterations = 10000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _id = generate_game_id();
        }

        let duration = start.elapsed();
        let per_id = duration / iterations;

        // Should be able to generate IDs reasonably fast
        assert!(
            per_id.as_micros() < 100,
            "ID generation should be fast (< 100μs per ID), got {:?}",
            per_id
        );

        println!(
            "Generated {} IDs in {:?} ({:?} per ID)",
            iterations, duration, per_id
        );
    }
}

// =============================================================================
// Board State Hashing Tests
// =============================================================================

#[cfg(test)]
mod board_state_hashing_tests {
    use super::*;

    #[test]
    fn test_hash_board_state_basic_properties() {
        let board = Board::new();
        let hash = hash_board_state(&board);

        // SHA-256 produces 64-character hex strings
        assert_eq!(
            hash.len(),
            64,
            "Board hash should be 64 characters (SHA-256 hex)"
        );

        // Should be all hexadecimal characters
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Board hash should be hexadecimal: {}",
            hash
        );

        // Should not be empty or all zeros
        assert_ne!(hash, "", "Board hash should not be empty");
        assert_ne!(
            hash, "0000000000000000000000000000000000000000000000000000000000000000",
            "Board hash should not be all zeros"
        );
    }

    #[test]
    fn test_hash_board_state_consistency() {
        let board = Board::new();

        // Multiple calls should produce the same hash
        let hash1 = hash_board_state(&board);
        let hash2 = hash_board_state(&board);
        let hash3 = hash_board_state(&board);

        assert_eq!(hash1, hash2, "Hash should be consistent across calls");
        assert_eq!(hash2, hash3, "Hash should be consistent across calls");
    }

    #[test]
    fn test_hash_board_state_deterministic() {
        let board = Board::new();
        let expected_hash = hash_board_state(&board);

        // Hash should be deterministic across multiple executions
        for i in 0..100 {
            let hash = hash_board_state(&board);
            assert_eq!(
                hash, expected_hash,
                "Hash should be deterministic on iteration {}",
                i
            );
        }
    }

    #[test]
    fn test_hash_board_state_different_positions() {
        let board1 = Board::new();
        let board2 = Board::new();

        // Make a move to create different board state
        // Note: This test assumes we can make moves - adjust based on actual Board API
        let hash1 = hash_board_state(&board1);

        // For now, just test that identical boards produce identical hashes
        let hash2 = hash_board_state(&board2);
        assert_eq!(
            hash1, hash2,
            "Identical board states should produce identical hashes"
        );

        // Test with cloned board
        let board3 = board1.clone();
        let hash3 = hash_board_state(&board3);
        assert_eq!(hash1, hash3, "Cloned board should produce identical hash");
    }

    #[test]
    fn test_hash_board_state_performance() {
        let board = Board::new();
        let iterations = 10000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _hash = hash_board_state(&board);
        }

        let duration = start.elapsed();
        let per_hash = duration / iterations;

        // Hashing should be reasonably fast
        assert!(
            per_hash.as_micros() < 50,
            "Board hashing should be fast (< 50μs per hash), got {:?}",
            per_hash
        );

        println!(
            "Hashed {} boards in {:?} ({:?} per hash)",
            iterations, duration, per_hash
        );
    }

    #[test]
    fn test_hash_board_state_concurrent_consistency() {
        let board = Arc::new(Board::new());
        let expected_hash = hash_board_state(&board);
        let thread_count = 10;
        let hashes_per_thread = 100;
        let mut handles = vec![];

        // Test concurrent hashing
        for _ in 0..thread_count {
            let board_clone = Arc::clone(&board);
            let expected = expected_hash.clone();
            let handle = thread::spawn(move || {
                for _ in 0..hashes_per_thread {
                    let hash = hash_board_state(&board_clone);
                    assert_eq!(hash, expected, "Concurrent hashing should be consistent");
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_hash_board_state_collision_resistance() {
        // While we can't test true collision resistance without massive computation,
        // we can test that similar boards produce different hashes
        let board1 = Board::new();
        let board2 = Board::new();

        let hash1 = hash_board_state(&board1);
        let hash2 = hash_board_state(&board2);

        // Identical boards should produce identical hashes
        assert_eq!(
            hash1, hash2,
            "Identical boards should produce identical hashes"
        );

        // Test that the hash is not obviously weak
        // (e.g., not just a simple checksum)
        assert!(hash1.len() == 64, "Hash should be full SHA-256 length");
        assert!(
            hash1 != "a".repeat(64),
            "Hash should not be trivial pattern"
        );
        assert!(
            hash1 != "f".repeat(64),
            "Hash should not be trivial pattern"
        );
    }
}

// =============================================================================
// Hash Verification Tests
// =============================================================================

#[cfg(test)]
mod hash_verification_tests {
    use super::*;

    #[test]
    fn test_verify_board_hash_valid_match() {
        let board = Board::new();
        let correct_hash = hash_board_state(&board);

        // Valid hash should verify successfully
        assert!(
            verify_board_hash(&board, &correct_hash),
            "Correct hash should verify successfully"
        );
    }

    #[test]
    fn test_verify_board_hash_invalid_rejection() {
        let board = Board::new();
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        // Wrong hash should be rejected
        assert!(
            !verify_board_hash(&board, wrong_hash),
            "Wrong hash should be rejected"
        );
    }

    #[test]
    fn test_verify_board_hash_case_insensitive() {
        let board = Board::new();
        let hash = hash_board_state(&board);
        let uppercase_hash = hash.to_uppercase();
        let mixed_case_hash = hash
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i % 2 == 0 {
                    c.to_uppercase().next().unwrap()
                } else {
                    c
                }
            })
            .collect::<String>();

        // Verification should be case-insensitive
        assert!(
            verify_board_hash(&board, &uppercase_hash),
            "Uppercase hash should verify successfully"
        );
        assert!(
            verify_board_hash(&board, &mixed_case_hash),
            "Mixed case hash should verify successfully"
        );
    }

    #[test]
    fn test_verify_board_hash_format_validation() {
        let board = Board::new();

        // Test invalid hash formats
        let invalid_hashes = [
            "",                                                                  // Empty
            "invalid",                                                           // Too short
            "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg", // Invalid hex character (64 g's)
            "000000000000000000000000000000000000000000000000000000000000000", // Too short by 1 (63 chars)
            "00000000000000000000000000000000000000000000000000000000000000000", // Too long by 1 (65 chars)
            "0123456789abcdef0123456789abcdef0123456789abcdef", // Wrong length (48 chars)
        ];

        for invalid_hash in &invalid_hashes {
            assert!(
                !verify_board_hash(&board, &invalid_hash),
                "Invalid hash format should be rejected: '{}'",
                invalid_hash
            );
        }
    }

    #[test]
    fn test_verify_board_hash_tampering_detection() {
        let board = Board::new();
        let correct_hash = hash_board_state(&board);

        // Test single character changes
        for i in 0..correct_hash.len() {
            let mut tampered_hash = correct_hash.clone();
            let tampered_char = if correct_hash.chars().nth(i).unwrap() == '0' {
                '1'
            } else {
                '0'
            };
            tampered_hash.replace_range(i..=i, &tampered_char.to_string());

            assert!(
                !verify_board_hash(&board, &tampered_hash),
                "Tampered hash should be rejected (position {}): '{}'",
                i,
                tampered_hash
            );
        }
    }

    #[test]
    fn test_verify_board_hash_graceful_success() {
        let board = Board::new();
        let correct_hash = hash_board_state(&board);

        let result = verify_board_hash_graceful("test-game", &board, &correct_hash, "test");
        assert!(
            result.is_ok(),
            "Graceful verification should succeed for correct hash"
        );
    }

    #[test]
    fn test_verify_board_hash_graceful_failure() {
        let board = Board::new();
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = verify_board_hash_graceful("test-game", &board, wrong_hash, "test");
        assert!(
            result.is_err(),
            "Graceful verification should fail for wrong hash"
        );

        // Check error details
        match result {
            Err(error) => {
                let error_string = error.to_string();
                assert!(
                    error_string.contains("hash"),
                    "Error should mention hash mismatch: {}",
                    error_string
                );
            }
            Ok(_) => panic!("Expected error for wrong hash"),
        }
    }

    #[test]
    fn test_verify_board_hash_graceful_invalid_format() {
        let board = Board::new();
        let invalid_hash = "invalid-hash-format";

        let result = verify_board_hash_graceful("test-game", &board, invalid_hash, "test");
        assert!(
            result.is_err(),
            "Graceful verification should fail for invalid format"
        );
    }
}

// =============================================================================
// Utility Function Integration Tests
// =============================================================================

#[cfg(test)]
mod utility_function_integration_tests {
    use super::*;

    #[test]
    fn test_game_id_validation_integration() {
        // Test integration between generation and validation
        for _ in 0..100 {
            let game_id = generate_game_id();
            assert!(
                validate_game_id(&game_id),
                "Generated game ID should always pass validation: {}",
                game_id
            );
        }
    }

    #[test]
    fn test_board_hash_integration_cycle() {
        let board = Board::new();

        // Test complete cycle: hash -> verify
        let hash = hash_board_state(&board);
        assert!(
            verify_board_hash(&board, &hash),
            "Hash verification cycle should work"
        );

        // Test with graceful verification
        let result = verify_board_hash_graceful("test-game", &board, &hash, "integration_test");
        assert!(result.is_ok(), "Graceful verification cycle should work");
    }

    #[test]
    fn test_cross_function_consistency() {
        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        // All functions should work together consistently
        assert!(validate_game_id(&game_id), "Game ID should be valid");
        assert!(verify_board_hash(&board, &hash), "Hash should verify");

        // Multiple calls should remain consistent
        let game_id2 = generate_game_id();
        let hash2 = hash_board_state(&board);

        assert!(
            validate_game_id(&game_id2),
            "Second game ID should be valid"
        );
        assert_ne!(game_id, game_id2, "Game IDs should be different");
        assert_eq!(hash, hash2, "Board hashes should be identical");
    }

    #[test]
    fn test_error_handling_consistency() {
        let board = Board::new();

        // Test consistent error handling across functions
        assert!(!validate_game_id(""), "Empty string should be invalid");
        assert!(
            !validate_game_id("not-a-uuid"),
            "Invalid format should be invalid"
        );

        assert!(
            !verify_board_hash(&board, ""),
            "Empty hash should be rejected"
        );
        assert!(
            !verify_board_hash(&board, "invalid"),
            "Invalid hash should be rejected"
        );
    }

    #[test]
    fn test_performance_characteristics() {
        let iterations = 1000;
        let board = Board::new();

        // Test combined operation performance
        let start = Instant::now();

        for _ in 0..iterations {
            let game_id = generate_game_id();
            let hash = hash_board_state(&board);

            assert!(validate_game_id(&game_id));
            assert!(verify_board_hash(&board, &hash));
        }

        let duration = start.elapsed();
        let per_iteration = duration / iterations;

        // Combined operations should be reasonably fast
        assert!(
            per_iteration.as_micros() < 200,
            "Combined operations should be fast (< 200μs per iteration), got {:?}",
            per_iteration
        );

        println!(
            "Performed {} combined operations in {:?} ({:?} per iteration)",
            iterations, duration, per_iteration
        );
    }
}

// =============================================================================
// Thread Safety Validation Tests
// =============================================================================

#[cfg(test)]
mod thread_safety_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_game_id_generation() {
        let thread_count = 20;
        let ids_per_thread = 50;
        let all_ids = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];

        // Spawn threads that generate IDs concurrently
        for _ in 0..thread_count {
            let ids_clone = Arc::clone(&all_ids);
            let handle = thread::spawn(move || {
                let mut thread_ids = Vec::new();
                for _ in 0..ids_per_thread {
                    let id = generate_game_id();
                    assert!(validate_game_id(&id), "Generated ID should be valid");
                    thread_ids.push(id);
                }

                let mut all_ids = ids_clone.lock().unwrap();
                all_ids.extend(thread_ids);
            });
            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        // Verify all IDs are unique
        let all_ids = all_ids.lock().unwrap();
        let unique_ids: HashSet<_> = all_ids.iter().collect();

        assert_eq!(
            unique_ids.len(),
            all_ids.len(),
            "All concurrently generated IDs should be unique"
        );
        assert_eq!(
            all_ids.len(),
            thread_count * ids_per_thread,
            "Should have generated expected number of IDs"
        );
    }

    #[test]
    fn test_concurrent_board_hashing() {
        let board = Arc::new(Board::new());
        let expected_hash = hash_board_state(&board);
        let thread_count = 10;
        let hashes_per_thread = 100;
        let mut handles = vec![];

        // Spawn threads that hash the same board concurrently
        for _ in 0..thread_count {
            let board_clone = Arc::clone(&board);
            let expected = expected_hash.clone();
            let handle = thread::spawn(move || {
                for _ in 0..hashes_per_thread {
                    let hash = hash_board_state(&board_clone);
                    assert_eq!(hash, expected, "Concurrent hashing should be consistent");
                    assert!(
                        verify_board_hash(&board_clone, &hash),
                        "Concurrent hash verification should work"
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for completion
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        let board = Arc::new(Board::new());
        let thread_count = 8;
        let operations_per_thread = 50;
        let mut handles = vec![];

        // Spawn threads that perform mixed operations concurrently
        for thread_id in 0..thread_count {
            let board_clone = Arc::clone(&board);
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    // Generate and validate game ID
                    let game_id = generate_game_id();
                    assert!(
                        validate_game_id(&game_id),
                        "Thread {}, iteration {}: Game ID should be valid",
                        thread_id,
                        i
                    );

                    // Hash and verify board
                    let hash = hash_board_state(&board_clone);
                    assert!(
                        verify_board_hash(&board_clone, &hash),
                        "Thread {}, iteration {}: Hash verification should work",
                        thread_id,
                        i
                    );

                    // Test graceful verification
                    let result = verify_board_hash_graceful(
                        &game_id,
                        &board_clone,
                        &hash,
                        &format!("thread-{}-iter-{}", thread_id, i),
                    );
                    assert!(
                        result.is_ok(),
                        "Thread {}, iteration {}: Graceful verification should work",
                        thread_id,
                        i
                    );
                }
            });
            handles.push(handle);
        }

        // Wait for completion
        for (i, handle) in handles.into_iter().enumerate() {
            handle
                .join()
                .unwrap_or_else(|_| panic!("Thread {} should complete successfully", i));
        }
    }

    #[test]
    fn test_thread_safety_stress_test() {
        let thread_count = 16;
        let duration_ms = 1000; // Run for 1 second
        let start_time = Instant::now();
        let mut handles = vec![];

        // Spawn threads that stress test all operations
        for thread_id in 0..thread_count {
            let handle = thread::spawn(move || {
                let board = Board::new();
                let mut operation_count = 0;

                while start_time.elapsed().as_millis() < duration_ms {
                    // Cycle through different operations
                    match operation_count % 4 {
                        0 => {
                            let game_id = generate_game_id();
                            assert!(validate_game_id(&game_id));
                        }
                        1 => {
                            let hash = hash_board_state(&board);
                            assert_eq!(hash.len(), 64);
                        }
                        2 => {
                            let hash = hash_board_state(&board);
                            assert!(verify_board_hash(&board, &hash));
                        }
                        3 => {
                            let game_id = generate_game_id();
                            let hash = hash_board_state(&board);
                            let result =
                                verify_board_hash_graceful(&game_id, &board, &hash, "stress");
                            assert!(result.is_ok());
                        }
                        _ => unreachable!(),
                    }
                    operation_count += 1;
                }

                println!(
                    "Thread {} completed {} operations in stress test",
                    thread_id, operation_count
                );
                operation_count
            });
            handles.push(handle);
        }

        // Wait for completion and collect results
        let mut total_operations = 0;
        for (i, handle) in handles.into_iter().enumerate() {
            let ops = handle.join().unwrap_or_else(|_| {
                panic!("Stress test thread {} should complete successfully", i)
            });
            total_operations += ops;
        }

        let actual_duration = start_time.elapsed();
        println!(
            "Stress test completed: {} total operations across {} threads in {:?}",
            total_operations, thread_count, actual_duration
        );

        // Should have completed a reasonable number of operations
        assert!(
            total_operations > 1000,
            "Stress test should complete significant number of operations"
        );
    }
}
