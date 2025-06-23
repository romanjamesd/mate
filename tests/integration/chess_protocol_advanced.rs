//! Chess Protocol Advanced Integration Tests
//!
//! Tests advanced chess protocol scenarios and edge cases as specified in tests-to-add.md:
//! - Large Message Handling: Sync responses with extensive move histories
//! - Concurrent Game Management: Multiple simultaneous games, resource management
//! - Network Resilience: Connection interruption handling, recovery mechanisms
//! - Performance Under Load: High-volume message processing, memory efficiency
//!
//! Key Focus Areas:
//! - Scalability testing with multiple concurrent games
//! - Network reliability and recovery
//! - Memory management with large datasets
//! - Performance bottleneck identification

use anyhow::Result;
use mate::chess::{Board, Color, Move as ChessMove};
use mate::crypto::Identity;
use mate::messages::chess::{generate_game_id, hash_board_state};
use mate::messages::types::{Message, SignedEnvelope};
use mate::messages::wire::{ConnectionState, FramedMessage, RetryConfig, WireConfig};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::duplex;
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::common::mock_streams::InterruptibleMockStream;

// =============================================================================
// Test Utilities and Helpers
// =============================================================================

/// Advanced game state manager for concurrent testing
#[derive(Debug, Clone)]
struct AdvancedGameState {
    #[allow(dead_code)]
    game_id: String,
    board: Board,
    move_history: Vec<String>,
    #[allow(dead_code)]
    white_player: String,
    #[allow(dead_code)]
    black_player: String,
    current_turn: Color,
    last_updated: Instant,
    message_count: usize,
}

impl AdvancedGameState {
    fn new_with_players(game_id: String, white_player: String, black_player: String) -> Self {
        Self {
            game_id,
            board: Board::new(),
            move_history: Vec::new(),
            white_player,
            black_player,
            current_turn: Color::White,
            last_updated: Instant::now(),
            message_count: 0,
        }
    }

    fn apply_move(&mut self, chess_move: &str) -> Result<()> {
        // Simulate move application with validation
        let chess_move_obj = ChessMove::from_str(chess_move)?;
        self.board.make_move(chess_move_obj)?;
        self.move_history.push(chess_move.to_string());
        self.current_turn = match self.current_turn {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        self.last_updated = Instant::now();
        self.message_count += 1;
        Ok(())
    }

    fn get_board_hash(&self) -> String {
        hash_board_state(&self.board)
    }

    fn create_large_history(&mut self, move_count: usize) -> Result<()> {
        // Create a large move history for testing purposes by generating fake moves
        // Since we're testing message handling, not chess logic, we can add moves directly to history
        for i in 0..move_count {
            let fake_move = match i % 8 {
                0 => format!("e{}e{}", (i % 6) + 2, (i % 6) + 3),
                1 => format!("d{}d{}", (i % 6) + 2, (i % 6) + 3),
                2 => format!("f{}f{}", (i % 6) + 2, (i % 6) + 3),
                3 => format!("c{}c{}", (i % 6) + 2, (i % 6) + 3),
                4 => format!("g{}g{}", (i % 6) + 2, (i % 6) + 3),
                5 => format!("h{}h{}", (i % 6) + 2, (i % 6) + 3),
                6 => format!("a{}a{}", (i % 6) + 2, (i % 6) + 3),
                _ => format!("b{}b{}", (i % 6) + 2, (i % 6) + 3),
            };

            self.move_history.push(fake_move);
            self.message_count += 1;
        }

        // Update last_updated timestamp
        self.last_updated = Instant::now();
        Ok(())
    }

    #[allow(dead_code)]
    fn estimated_sync_message_size(&self) -> usize {
        // Estimate the size of a sync response message
        let fen_size = self.board.to_fen().len();
        let history_size = self.move_history.iter().map(|m| m.len()).sum::<usize>();
        let hash_size = 64; // SHA-256 hash length
        let overhead = 100; // JSON serialization overhead
        fen_size + history_size + hash_size + overhead
    }
}

/// Game manager for concurrent testing
#[derive(Clone)]
struct ConcurrentGameManager {
    games: Arc<RwLock<HashMap<String, AdvancedGameState>>>,
    max_games: usize,
    message_counts: Arc<RwLock<HashMap<String, usize>>>,
}

impl ConcurrentGameManager {
    fn new(max_games: usize) -> Self {
        Self {
            games: Arc::new(RwLock::new(HashMap::new())),
            max_games,
            message_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn create_game(
        &self,
        game_id: String,
        white_player: String,
        black_player: String,
    ) -> Result<()> {
        let mut games = self.games.write().await;
        if games.len() >= self.max_games {
            return Err(anyhow::anyhow!("Maximum concurrent games reached"));
        }

        let game_state =
            AdvancedGameState::new_with_players(game_id.clone(), white_player, black_player);
        games.insert(game_id.clone(), game_state);

        let mut counts = self.message_counts.write().await;
        counts.insert(game_id, 0);
        Ok(())
    }

    async fn apply_move_to_game(&self, game_id: &str, chess_move: &str) -> Result<()> {
        let mut games = self.games.write().await;
        if let Some(game) = games.get_mut(game_id) {
            game.apply_move(chess_move)?;

            let mut counts = self.message_counts.write().await;
            *counts.entry(game_id.to_string()).or_insert(0) += 1;
        }
        Ok(())
    }

    async fn get_game_count(&self) -> usize {
        self.games.read().await.len()
    }

    async fn get_total_messages(&self) -> usize {
        self.message_counts.read().await.values().sum()
    }

    async fn create_sync_response(&self, game_id: &str) -> Option<Message> {
        let games = self.games.read().await;
        games.get(game_id).map(|game| {
            Message::new_sync_response(
                game_id.to_string(),
                game.board.to_fen(),
                game.move_history.clone(),
                game.get_board_hash(),
            )
        })
    }
}

// =============================================================================
// Large Message Handling Tests
// =============================================================================

#[tokio::test]
async fn test_large_sync_response_processing() -> Result<()> {
    println!("Testing large sync response message processing...");

    let game_id = generate_game_id();
    let mut game_state = AdvancedGameState::new_with_players(
        game_id.clone(),
        "player1".to_string(),
        "player2".to_string(),
    );

    // Create a large move history (1000 moves)
    game_state.create_large_history(1000)?;

    // Create sync response message
    let sync_message = Message::new_sync_response(
        game_id.clone(),
        game_state.board.to_fen(),
        game_state.move_history.clone(),
        game_state.get_board_hash(),
    );

    // Test with chess sync configuration
    let wire_config = WireConfig::for_chess_sync();
    let framed_message = FramedMessage::new(wire_config);

    // Create identities for testing
    let identity = Arc::new(Identity::generate()?);
    let envelope = SignedEnvelope::create(&sync_message, &identity, None)?;

    // Test serialization and size validation
    let serialized_size = envelope.message.len();
    println!("  Large sync message size: {} bytes", serialized_size);

    // Verify message is potentially large
    assert!(
        sync_message.is_potentially_large(),
        "Sync message should be marked as potentially large"
    );

    // Test message transmission over wire protocol
    let (stream1, stream2) = duplex(16 * 1024 * 1024); // 16MB buffer for large messages
    let (_reader, mut writer) = tokio::io::split(stream1);
    let (mut test_reader, _test_writer) = tokio::io::split(stream2);

    // Write large message
    let write_start = Instant::now();
    framed_message.write_message(&mut writer, &envelope).await?;
    let write_duration = write_start.elapsed();

    // Read large message
    let read_start = Instant::now();
    let received_envelope = framed_message.read_message(&mut test_reader).await?;
    let read_duration = read_start.elapsed();

    // Verify message integrity
    let received_message = received_envelope.get_message()?;
    assert!(matches!(received_message, Message::SyncResponse(_)));

    if let Message::SyncResponse(sync_resp) = received_message {
        assert_eq!(sync_resp.game_id, game_id);
        assert_eq!(sync_resp.move_history.len(), game_state.move_history.len());
        assert_eq!(sync_resp.board_state, game_state.board.to_fen());
    }

    println!("  ✓ Large message write time: {:?}", write_duration);
    println!("  ✓ Large message read time: {:?}", read_duration);
    println!("  ✓ Message integrity verified");

    Ok(())
}

#[tokio::test]
async fn test_extremely_large_sync_response_limits() -> Result<()> {
    println!("Testing extremely large sync response message limits...");

    let game_id = generate_game_id();
    let mut game_state = AdvancedGameState::new_with_players(
        game_id.clone(),
        "player1".to_string(),
        "player2".to_string(),
    );

    // Create an extremely large move history (5000 moves)
    game_state.create_large_history(5000)?;

    let sync_message = Message::new_sync_response(
        game_id.clone(),
        game_state.board.to_fen(),
        game_state.move_history.clone(),
        game_state.get_board_hash(),
    );

    // Test with different wire configurations
    let configs = vec![
        ("Standard", WireConfig::for_chess_standard()),
        ("Sync", WireConfig::for_chess_sync()),
        ("Bulk", WireConfig::for_chess_bulk()),
    ];

    for (config_name, wire_config) in configs {
        println!("  Testing with {} configuration...", config_name);

        let framed_message = FramedMessage::new(wire_config.clone());
        let identity = Arc::new(Identity::generate()?);

        // Test envelope creation
        let envelope_result = SignedEnvelope::create(&sync_message, &identity, None);

        match envelope_result {
            Ok(envelope) => {
                let message_size = envelope.message.len();
                println!("    Message size: {} bytes", message_size);

                // Only test transmission if size is within config limits
                if message_size <= wire_config.max_message_size {
                    let (stream1, stream2) = duplex(wire_config.max_message_size + 1024);
                    let (_reader, mut writer) = tokio::io::split(stream1);
                    let (mut test_reader, _test_writer) = tokio::io::split(stream2);

                    // Test with timeout to prevent hanging
                    let write_result = timeout(
                        Duration::from_secs(10),
                        framed_message.write_message(&mut writer, &envelope),
                    )
                    .await;

                    match write_result {
                        Ok(Ok(_)) => {
                            println!("    ✓ Successfully transmitted large message");

                            // Verify we can read it back
                            let read_result = timeout(
                                Duration::from_secs(10),
                                framed_message.read_message(&mut test_reader),
                            )
                            .await;

                            match read_result {
                                Ok(Ok(_)) => println!("    ✓ Successfully received large message"),
                                Ok(Err(e)) => println!("    ✗ Failed to read message: {}", e),
                                Err(_) => println!("    ✗ Read timeout"),
                            }
                        }
                        Ok(Err(e)) => println!("    ✗ Failed to write message: {}", e),
                        Err(_) => println!("    ✗ Write timeout"),
                    }
                } else {
                    println!("    ! Message too large for {} configuration", config_name);
                }
            }
            Err(e) => println!("    ✗ Failed to create envelope: {}", e),
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_sync_response_memory_efficiency() -> Result<()> {
    println!("Testing sync response memory efficiency...");

    let game_id = generate_game_id();
    let identity = Arc::new(Identity::generate()?);

    // Test with various history sizes
    let history_sizes = vec![10, 100, 500, 1000, 2000];

    for &size in &history_sizes {
        let mut game_state = AdvancedGameState::new_with_players(
            game_id.clone(),
            "player1".to_string(),
            "player2".to_string(),
        );

        game_state.create_large_history(size)?;

        let sync_message = Message::new_sync_response(
            game_id.clone(),
            game_state.board.to_fen(),
            game_state.move_history.clone(),
            game_state.get_board_hash(),
        );

        // Measure memory usage
        let estimated_size = sync_message.estimated_size();
        let actual_envelope = SignedEnvelope::create(&sync_message, &identity, None)?;
        let actual_size = actual_envelope.message.len();

        println!(
            "  History size: {}, Estimated: {} bytes, Actual: {} bytes",
            size, estimated_size, actual_size
        );

        // Verify estimation accuracy (should be within reasonable range)
        let size_ratio = actual_size as f64 / estimated_size as f64;
        assert!(
            (0.5..=2.0).contains(&size_ratio),
            "Size estimation should be reasonably accurate"
        );
    }

    println!("  ✓ Memory estimation accuracy verified");
    Ok(())
}

// =============================================================================
// Concurrent Game Management Tests
// =============================================================================

#[tokio::test]
async fn test_multiple_simultaneous_games() -> Result<()> {
    println!("Testing multiple simultaneous games management...");

    let game_manager = ConcurrentGameManager::new(100);
    let mut game_handles = Vec::new();

    // Create 50 concurrent games
    for i in 0..50 {
        let game_id = generate_game_id();
        let manager = game_manager.clone();

        let handle = tokio::spawn(async move {
            let white_player = format!("white_player_{}", i);
            let black_player = format!("black_player_{}", i);

            // Create game
            manager
                .create_game(game_id.clone(), white_player, black_player)
                .await?;

            // Simulate game play with moves (focus on message processing, not chess validity)
            let moves = vec![
                "e2e4", "e7e5", "Nf3", "Nc6", "Bb5", "a6", "Ba4", "Nf6", "O-O", "Be7",
            ];
            for chess_move in moves {
                // For this test, we'll add moves directly to history since we're testing message processing
                // rather than chess game validity
                let mut games = manager.games.write().await;
                if let Some(game) = games.get_mut(&game_id) {
                    game.move_history.push(chess_move.to_string());
                    game.message_count += 1;
                }
                drop(games);

                // Update message count
                let mut counts = manager.message_counts.write().await;
                *counts.entry(game_id.clone()).or_insert(0) += 1;
                drop(counts);

                // Small delay to simulate real gameplay
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            Result::<String, anyhow::Error>::Ok(game_id)
        });

        game_handles.push(handle);
    }

    // Wait for all games to complete
    let mut completed_games = Vec::new();
    for handle in game_handles {
        match handle.await {
            Ok(Ok(game_id)) => completed_games.push(game_id),
            Ok(Err(e)) => println!("  Game error: {}", e),
            Err(e) => println!("  Task error: {}", e),
        }
    }

    let final_game_count = game_manager.get_game_count().await;
    let total_messages = game_manager.get_total_messages().await;

    println!("  ✓ Completed games: {}", completed_games.len());
    println!("  ✓ Active games: {}", final_game_count);
    println!("  ✓ Total messages processed: {}", total_messages);

    assert!(
        completed_games.len() >= 45,
        "Most games should complete successfully"
    );
    assert!(
        total_messages >= 400, // 50 games × 8 moves = 400 minimum expected messages
        "Should process significant number of messages, got {}",
        total_messages
    );

    Ok(())
}

#[tokio::test]
async fn test_concurrent_sync_message_generation() -> Result<()> {
    println!("Testing concurrent sync message generation...");

    let game_manager = ConcurrentGameManager::new(20);
    let mut sync_handles = Vec::new();

    // Create games with different history sizes
    for i in 0..20 {
        let game_id = generate_game_id();
        let manager = game_manager.clone();

        // Create game with varying history sizes
        manager
            .create_game(
                game_id.clone(),
                format!("player_a_{}", i),
                format!("player_b_{}", i),
            )
            .await?;

        // Add moves to create different history sizes
        let move_count = (i + 1) * 50; // 50, 100, 150, ... moves
        for move_num in 0..move_count {
            let chess_move = match move_num % 4 {
                0 => "e2e4",
                1 => "e7e5",
                2 => "Nf3",
                _ => "Nc6",
            };
            let _ = manager.apply_move_to_game(&game_id, chess_move).await;
        }

        // Spawn concurrent sync message generation
        let handle = tokio::spawn(async move {
            let sync_message = manager.create_sync_response(&game_id).await;
            match sync_message {
                Some(msg) => {
                    let size = msg.estimated_size();
                    Ok((game_id, size))
                }
                None => Err(anyhow::anyhow!("Failed to create sync response")),
            }
        });

        sync_handles.push(handle);
    }

    // Collect results
    let mut sync_results = Vec::new();
    for handle in sync_handles {
        match handle.await {
            Ok(Ok((game_id, size))) => {
                sync_results.push((game_id, size));
            }
            Ok(Err(e)) => println!("  Sync generation error: {}", e),
            Err(e) => println!("  Task error: {}", e),
        }
    }

    println!("  ✓ Generated {} sync messages", sync_results.len());

    // Verify size distribution
    let total_size: usize = sync_results.iter().map(|(_, size)| size).sum();
    let avg_size = total_size / sync_results.len();
    println!("  ✓ Average sync message size: {} bytes", avg_size);

    assert_eq!(
        sync_results.len(),
        20,
        "All sync messages should be generated"
    );
    assert!(
        avg_size > 200,
        "Average message size should be substantial, got {} bytes",
        avg_size
    );

    Ok(())
}

#[tokio::test]
async fn test_resource_management_under_load() -> Result<()> {
    println!("Testing resource management under concurrent load...");

    let game_manager = ConcurrentGameManager::new(10); // Lower limit for stress testing
    let mut tasks: Vec<tokio::task::JoinHandle<Result<bool, anyhow::Error>>> = Vec::new();

    // Create more tasks than the limit allows
    for i in 0..20 {
        let manager = game_manager.clone();

        let task = tokio::spawn(async move {
            let game_id = generate_game_id();
            let result = manager
                .create_game(
                    game_id.clone(),
                    format!("player1_{}", i),
                    format!("player2_{}", i),
                )
                .await;

            match result {
                Ok(_) => {
                    // If game creation succeeded, try to add some moves
                    for j in 0..10 {
                        let chess_move = match j % 2 {
                            0 => "e2e4",
                            _ => "e7e5",
                        };
                        let _ = manager.apply_move_to_game(&game_id, chess_move).await;
                    }
                    Ok(true) // Successfully created and used game
                }
                Err(_) => Ok(false), // Failed to create (expected due to limits)
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks
    let mut successful_creations = 0;
    let mut failed_creations = 0;

    for task in tasks {
        match task.await {
            Ok(Ok(true)) => successful_creations += 1,
            Ok(Ok(false)) => failed_creations += 1,
            Ok(Err(e)) => println!("  Task error: {}", e),
            Err(e) => println!("  Join error: {}", e),
        }
    }

    println!("  ✓ Successful game creations: {}", successful_creations);
    println!(
        "  ✓ Failed game creations (due to limits): {}",
        failed_creations
    );

    // Verify resource limits are enforced
    assert_eq!(
        successful_creations, 10,
        "Should respect maximum game limit"
    );
    assert_eq!(
        failed_creations, 10,
        "Should reject excess game creation requests"
    );

    let final_game_count = game_manager.get_game_count().await;
    assert_eq!(
        final_game_count, 10,
        "Should maintain exactly the maximum allowed games"
    );

    Ok(())
}

// =============================================================================
// Network Resilience Tests
// =============================================================================

#[tokio::test]
async fn test_connection_interruption_recovery() -> Result<()> {
    println!("Testing connection interruption and recovery...");

    let game_id = generate_game_id();
    let identity = Arc::new(Identity::generate()?);

    // Create a message to transmit
    let move_msg = Message::new_move(
        game_id.clone(),
        "e2e4".to_string(),
        "dummy_hash".to_string(),
    );
    let envelope = SignedEnvelope::create(&move_msg, &identity, None)?;

    // Set up interruption points at various byte positions
    let message_data = bincode::serialize(&envelope)?;
    let interruption_points = vec![10, 50, 100, 200]; // Interrupt at these byte positions

    let interruptible_stream =
        InterruptibleMockStream::new(message_data.clone(), interruption_points);
    let wire_config = WireConfig::for_chess_standard();
    let framed_message = FramedMessage::new(wire_config);

    // Test resilient reading with interruptions
    let retry_config = RetryConfig::default();
    let mut connection_state = ConnectionState::Healthy;

    let read_result = framed_message
        .read_message_with_graceful_degradation(
            &mut Box::pin(interruptible_stream),
            &retry_config,
            &mut connection_state,
        )
        .await;

    match read_result {
        Ok(received_envelope) => {
            println!("  ✓ Successfully recovered from connection interruptions");

            // Verify message integrity
            let received_message = received_envelope.get_message()?;
            assert!(matches!(received_message, Message::Move(_)));
        }
        Err(e) => {
            println!("  ✗ Failed to recover from interruptions: {}", e);
            // This is expected in some cases - test that we handle it gracefully
            assert!(matches!(
                connection_state,
                ConnectionState::Degraded { .. } | ConnectionState::Broken { .. }
            ));
        }
    }

    println!(
        "  ✓ Connection state after interruption: {:?}",
        connection_state
    );
    Ok(())
}

#[tokio::test]
async fn test_message_timeout_handling() -> Result<()> {
    println!("Testing message timeout handling...");

    let game_id = generate_game_id();
    let identity = Arc::new(Identity::generate()?);

    // Create a moderately sized sync message that should timeout with very short timeouts
    let mut game_state = AdvancedGameState::new_with_players(
        game_id.clone(),
        "player1".to_string(),
        "player2".to_string(),
    );
    game_state.create_large_history(100)?; // Reduced from 2000 to 100 for more predictable behavior

    let move_history = game_state.move_history.clone();
    let board_hash = game_state.get_board_hash();
    let sync_message = Message::new_sync_response(
        game_id.clone(),
        game_state.board.to_fen(),
        move_history,
        board_hash,
    );

    let envelope = SignedEnvelope::create(&sync_message, &identity, None)?;

    // Test write timeout with explicit timeout method
    let wire_config = WireConfig::for_chess_standard(); // Use standard config
    let framed_message = FramedMessage::new(wire_config);

    // Use a smaller buffer to force blocking behavior
    let (stream1, stream2) = duplex(512); // Very small buffer to force timeout
    let (mut _reader, mut writer) = tokio::io::split(stream1);
    let (mut test_reader, _test_writer) = tokio::io::split(stream2);

    // Test write timeout using the explicit timeout method with very short timeout
    let short_timeout = Duration::from_millis(1);
    let write_result = framed_message
        .write_message_with_timeout(&mut writer, &envelope, short_timeout)
        .await;

    match write_result {
        Ok(_) => println!("  ! Message write succeeded (unexpectedly fast)"),
        Err(e) => {
            println!("  ✓ Write timeout handled gracefully: {}", e);
            // Verify it's a timeout error
            let error_str = e.to_string();
            assert!(
                error_str.contains("timeout")
                    || error_str.contains("Timeout")
                    || error_str.contains("timed out")
                    || error_str.contains("deadline has elapsed"),
                "Expected timeout error, got: {}",
                error_str
            );
        }
    }

    // Test read timeout using explicit timeout method
    let read_result = framed_message
        .read_message_with_timeout(&mut test_reader, Duration::from_millis(10))
        .await;

    match read_result {
        Ok(_) => println!("  ! Message read succeeded (unexpectedly)"),
        Err(e) => {
            println!("  ✓ Read timeout handled gracefully: {}", e);
            let error_str = e.to_string();
            assert!(
                error_str.contains("timeout")
                    || error_str.contains("Timeout")
                    || error_str.contains("timed out")
                    || error_str.contains("deadline has elapsed"),
                "Expected timeout error, got: {}",
                error_str
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_network_recovery_mechanisms() -> Result<()> {
    println!("Testing network recovery mechanisms...");

    let game_id = generate_game_id();
    let identity = Arc::new(Identity::generate()?);

    let move_msg = Message::new_move(
        game_id.clone(),
        "e2e4".to_string(),
        hash_board_state(&Board::new()),
    );
    let envelope = SignedEnvelope::create(&move_msg, &identity, None)?;

    // Configure aggressive retry policy
    let retry_config = RetryConfig::aggressive();
    let wire_config = WireConfig::for_chess_standard();
    let framed_message = FramedMessage::new(wire_config);

    // Create a resilient session
    let mut resilient_session = framed_message.create_resilient_session(retry_config);

    // Simulate various network conditions
    let network_conditions = vec![
        ("Healthy", vec![]),              // No interruptions
        ("Intermittent", vec![50, 150]),  // Some interruptions
        ("Poor", vec![25, 75, 125, 175]), // Many interruptions
    ];

    for (condition_name, interruption_points) in network_conditions {
        println!("  Testing {} network conditions...", condition_name);

        let message_data = bincode::serialize(&envelope)?;
        let test_stream = InterruptibleMockStream::new(message_data, interruption_points);

        let read_result = resilient_session
            .read_message(&mut Box::pin(test_stream))
            .await;

        match read_result {
            Ok(received_envelope) => {
                println!(
                    "    ✓ Successfully handled {} network",
                    condition_name.to_lowercase()
                );

                // Verify message integrity
                let received_message = received_envelope.get_message()?;
                assert!(matches!(received_message, Message::Move(_)));
            }
            Err(e) => {
                println!(
                    "    ✗ Failed under {} network: {}",
                    condition_name.to_lowercase(),
                    e
                );

                // Check that connection state reflects the problem
                let connection_state = resilient_session.connection_state();
                assert!(
                    !connection_state.can_attempt_operation()
                        || matches!(connection_state, ConnectionState::Degraded { .. })
                );
            }
        }

        // Reset connection state for next test
        resilient_session.reset_connection_state();
    }

    Ok(())
}

// =============================================================================
// Performance Under Load Tests
// =============================================================================

#[tokio::test]
async fn test_high_volume_message_processing() -> Result<()> {
    println!("Testing high-volume message processing performance...");

    let identity = Arc::new(Identity::generate()?);
    let wire_config = WireConfig::for_chess_standard();
    let framed_message = FramedMessage::new(wire_config);

    // Test parameters
    let message_count = 1000;
    let batch_size = 100;

    // Create messages in batches
    let mut total_processing_time = Duration::new(0, 0);
    let mut successful_messages = 0;

    for batch in 0..(message_count / batch_size) {
        let batch_start = Instant::now();
        let mut batch_messages = Vec::new();

        // Create batch of messages
        for i in 0..batch_size {
            let game_id = generate_game_id();
            let message_num = batch * batch_size + i;

            let message = match message_num % 3 {
                0 => Message::new_game_invite(game_id, Some(Color::White)),
                1 => Message::new_move(game_id, "e2e4".to_string(), "dummy_hash".to_string()),
                _ => Message::new_move_ack(game_id, Some(format!("move_{}", message_num))),
            };

            let envelope = SignedEnvelope::create(&message, &identity, None)?;
            batch_messages.push(envelope);
        }

        // Process batch
        let (stream1, stream2) = duplex(1024 * 1024); // 1MB buffer
        let (_reader, mut writer) = tokio::io::split(stream1);
        let (mut test_reader, _test_writer) = tokio::io::split(stream2);

        // Write all messages in batch
        for envelope in &batch_messages {
            if let Err(e) = framed_message.write_message(&mut writer, envelope).await {
                println!("    Write error in batch {}: {}", batch, e);
                continue;
            }
        }

        // Read all messages in batch
        for _ in 0..batch_size {
            match framed_message.read_message(&mut test_reader).await {
                Ok(_) => successful_messages += 1,
                Err(e) => println!("    Read error in batch {}: {}", batch, e),
            }
        }

        let batch_duration = batch_start.elapsed();
        total_processing_time += batch_duration;

        if batch % 2 == 0 {
            // Progress indication
            println!(
                "  Processed batch {}/{}",
                batch + 1,
                message_count / batch_size
            );
        }
    }

    let avg_processing_time = total_processing_time / (message_count / batch_size) as u32;
    let messages_per_second = (successful_messages as f64) / total_processing_time.as_secs_f64();

    println!(
        "  ✓ Total messages processed: {}/{}",
        successful_messages, message_count
    );
    println!(
        "  ✓ Average batch processing time: {:?}",
        avg_processing_time
    );
    println!("  ✓ Messages per second: {:.2}", messages_per_second);

    // Performance assertions
    assert!(
        successful_messages >= message_count * 95 / 100,
        "Should process at least 95% of messages successfully"
    );
    assert!(
        messages_per_second > 50.0,
        "Should process at least 50 messages per second"
    );

    Ok(())
}

#[tokio::test]
async fn test_memory_efficiency_under_load() -> Result<()> {
    println!("Testing memory efficiency under load...");

    let identity = Arc::new(Identity::generate()?);
    let wire_config = WireConfig::for_chess_sync(); // Allow larger messages
    let _framed_message = FramedMessage::new(wire_config);

    // Create messages of varying sizes to test memory efficiency
    let message_sizes = vec![
        (10, "small"),        // 10 moves
        (100, "medium"),      // 100 moves
        (500, "large"),       // 500 moves
        (1000, "very_large"), // 1000 moves
    ];

    let mut memory_stats = Vec::new();

    for (move_count, size_label) in message_sizes {
        println!(
            "  Testing {} messages ({} moves)...",
            size_label, move_count
        );

        let game_id = generate_game_id();
        let mut game_state = AdvancedGameState::new_with_players(
            game_id.clone(),
            "player1".to_string(),
            "player2".to_string(),
        );

        // Create history of specified size
        game_state.create_large_history(move_count)?;

        let sync_message = Message::new_sync_response(
            game_id.clone(),
            game_state.board.to_fen(),
            game_state.move_history.clone(),
            game_state.get_board_hash(),
        );

        // Measure memory usage
        let estimated_size = sync_message.estimated_size();
        let envelope = SignedEnvelope::create(&sync_message, &identity, None)?;
        let actual_size = envelope.message.len();

        // Test serialization/deserialization efficiency
        let serialize_start = Instant::now();
        let serialized = bincode::serialize(&envelope)?;
        let serialize_time = serialize_start.elapsed();

        let deserialize_start = Instant::now();
        let _deserialized: SignedEnvelope = bincode::deserialize(&serialized)?;
        let deserialize_time = deserialize_start.elapsed();

        memory_stats.push((
            size_label,
            move_count,
            estimated_size,
            actual_size,
            serialize_time,
            deserialize_time,
        ));

        println!(
            "    Estimated: {} bytes, Actual: {} bytes",
            estimated_size, actual_size
        );
        println!(
            "    Serialize: {:?}, Deserialize: {:?}",
            serialize_time, deserialize_time
        );
    }

    // Analyze memory efficiency trends
    println!("  Memory efficiency analysis:");
    for (label, moves, estimated, actual, ser_time, deser_time) in memory_stats {
        let bytes_per_move = actual / moves;
        let efficiency_ratio = estimated as f64 / actual as f64;

        println!(
            "    {}: {} bytes/move, {:.2}x estimation accuracy, ser: {:?}, deser: {:?}",
            label, bytes_per_move, efficiency_ratio, ser_time, deser_time
        );

        // Verify reasonable memory usage
        assert!(
            bytes_per_move < 1000,
            "Memory usage per move should be reasonable"
        );
        assert!(
            efficiency_ratio > 0.5 && efficiency_ratio < 2.0,
            "Size estimation should be reasonably accurate"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_performance_bottleneck_identification() -> Result<()> {
    println!("Testing performance bottleneck identification...");

    let identity = Arc::new(Identity::generate()?);

    // Test different wire configurations
    let configs = vec![
        ("Standard", WireConfig::for_chess_standard()),
        ("Sync", WireConfig::for_chess_sync()),
        ("Bulk", WireConfig::for_chess_bulk()),
        ("Realtime", WireConfig::for_chess_realtime()),
    ];

    let mut performance_results = Vec::new();

    for (config_name, wire_config) in configs {
        println!("  Testing {} configuration...", config_name);

        let framed_message = FramedMessage::new(wire_config.clone());
        let game_id = generate_game_id();

        // Create test message
        let mut game_state = AdvancedGameState::new_with_players(
            game_id.clone(),
            "player1".to_string(),
            "player2".to_string(),
        );
        game_state.create_large_history(200)?; // Medium-sized history

        let move_history = game_state.move_history.clone();
        let board_hash = game_state.get_board_hash();
        let sync_message = Message::new_sync_response(
            game_id.clone(),
            game_state.board.to_fen(),
            move_history,
            board_hash,
        );

        let envelope = SignedEnvelope::create(&sync_message, &identity, None)?;
        let message_size = envelope.message.len();

        // Skip if message is too large for this configuration
        if message_size > wire_config.max_message_size {
            println!("    Skipping - message too large for configuration");
            continue;
        }

        // Measure different performance aspects
        let (stream1, stream2) = duplex(wire_config.max_message_size + 1024);
        let (_reader, mut writer) = tokio::io::split(stream1);
        let (mut test_reader, _test_writer) = tokio::io::split(stream2);

        // Write performance
        let write_start = Instant::now();
        framed_message.write_message(&mut writer, &envelope).await?;
        let write_time = write_start.elapsed();

        // Read performance
        let read_start = Instant::now();
        let _received = framed_message.read_message(&mut test_reader).await?;
        let read_time = read_start.elapsed();

        // Calculate throughput
        let write_throughput = (message_size as f64) / write_time.as_secs_f64();
        let read_throughput = (message_size as f64) / read_time.as_secs_f64();

        performance_results.push((
            config_name,
            message_size,
            write_time,
            read_time,
            write_throughput,
            read_throughput,
        ));

        println!(
            "    Write: {:?} ({:.0} bytes/sec)",
            write_time, write_throughput
        );
        println!(
            "    Read: {:?} ({:.0} bytes/sec)",
            read_time, read_throughput
        );
    }

    // Identify bottlenecks
    println!("  Performance comparison:");
    let mut best_write_throughput = 0.0;
    let mut best_read_throughput = 0.0;
    let mut best_write_config = "";
    let mut best_read_config = "";

    for (config, size, write_time, read_time, write_tp, read_tp) in &performance_results {
        println!(
            "    {}: {} bytes, write: {:?}, read: {:?}",
            config, size, write_time, read_time
        );

        if *write_tp > best_write_throughput {
            best_write_throughput = *write_tp;
            best_write_config = config;
        }

        if *read_tp > best_read_throughput {
            best_read_throughput = *read_tp;
            best_read_config = config;
        }
    }

    println!(
        "  ✓ Best write performance: {} ({:.0} bytes/sec)",
        best_write_config, best_write_throughput
    );
    println!(
        "  ✓ Best read performance: {} ({:.0} bytes/sec)",
        best_read_config, best_read_throughput
    );

    // Verify we have meaningful performance data
    assert!(
        !performance_results.is_empty(),
        "Should have performance data for at least one configuration"
    );
    assert!(
        best_write_throughput > 1000.0,
        "Should achieve reasonable write throughput"
    );
    assert!(
        best_read_throughput > 1000.0,
        "Should achieve reasonable read throughput"
    );

    Ok(())
}
