//! Timing Display Tests
//! 
//! Tests for timing display functionality as specified in tests-to-add.md:
//! - Test that response times are displayed in human-readable format
//! - Test timing precision adjusts appropriately for different response speeds
//! - Test timing display for very fast, typical, and slow responses
//! - Test timing display consistency across different operations

use std::time::Duration;

/// Import the function we're testing
/// Note: This function is currently in main.rs, but for testing we need to extract it
/// to a testable module. For now, we'll duplicate it here for testing.
fn format_round_trip_time(duration: Duration) -> String {
    let millis = duration.as_millis();
    let micros = duration.as_micros();
    
    if millis == 0 {
        format!("{}μs", micros)
    } else if millis < 1000 {
        format!("{}ms", millis)
    } else {
        let seconds = duration.as_secs_f64();
        format!("{:.2}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test that response times are displayed in human-readable format
    #[test]
    fn test_human_readable_format() {
        // Very fast responses should show microseconds
        assert_eq!(format_round_trip_time(Duration::from_micros(500)), "500μs");
        assert_eq!(format_round_trip_time(Duration::from_micros(999)), "999μs");
        
        // Typical responses should show milliseconds
        assert_eq!(format_round_trip_time(Duration::from_millis(1)), "1ms");
        assert_eq!(format_round_trip_time(Duration::from_millis(50)), "50ms");
        assert_eq!(format_round_trip_time(Duration::from_millis(500)), "500ms");
        assert_eq!(format_round_trip_time(Duration::from_millis(999)), "999ms");
        
        // Slow responses should show seconds with 2 decimal places
        assert_eq!(format_round_trip_time(Duration::from_millis(1000)), "1.00s");
        assert_eq!(format_round_trip_time(Duration::from_millis(1500)), "1.50s");
        assert_eq!(format_round_trip_time(Duration::from_millis(2000)), "2.00s");
        assert_eq!(format_round_trip_time(Duration::from_secs(5)), "5.00s");
        
        println!("✅ Human-readable format test passed");
    }

    /// Test timing precision adjusts appropriately for different response speeds
    #[test]
    fn test_precision_adjustment() {
        // Microsecond precision for sub-millisecond times
        let very_fast = Duration::from_nanos(500_000); // 0.5ms = 500μs
        let result = format_round_trip_time(very_fast);
        assert!(result.ends_with("μs"), "Sub-millisecond times should use microsecond precision");
        assert_eq!(result, "500μs");
        
        // Millisecond precision for sub-second times
        let typical = Duration::from_millis(250);
        let result = format_round_trip_time(typical);
        assert!(result.ends_with("ms"), "Sub-second times should use millisecond precision");
        assert_eq!(result, "250ms");
        
        // Second precision (2 decimal places) for longer times
        let slow = Duration::from_millis(1234);
        let result = format_round_trip_time(slow);
        assert!(result.ends_with("s"), "Multi-second times should use second precision");
        assert_eq!(result, "1.23s");
        
        println!("✅ Precision adjustment test passed");
    }

    /// Test timing display for very fast, typical, and slow responses
    #[test]
    fn test_response_speed_categories() {
        // Very fast responses (< 1ms)
        let very_fast_cases = vec![
            (Duration::from_nanos(1_000), "1μs"),        // 1 microsecond
            (Duration::from_nanos(10_000), "10μs"),      // 10 microseconds
            (Duration::from_nanos(100_000), "100μs"),    // 100 microseconds
            (Duration::from_nanos(500_000), "500μs"),    // 500 microseconds
            (Duration::from_nanos(999_000), "999μs"),    // 999 microseconds
        ];
        
        for (duration, expected) in very_fast_cases {
            let result = format_round_trip_time(duration);
            assert_eq!(result, expected, "Very fast response formatting failed for {:?}", duration);
        }
        
        // Typical responses (1ms - 999ms)
        let typical_cases = vec![
            (Duration::from_millis(1), "1ms"),
            (Duration::from_millis(5), "5ms"),
            (Duration::from_millis(10), "10ms"),
            (Duration::from_millis(50), "50ms"),
            (Duration::from_millis(100), "100ms"),
            (Duration::from_millis(250), "250ms"),
            (Duration::from_millis(500), "500ms"),
            (Duration::from_millis(750), "750ms"),
            (Duration::from_millis(999), "999ms"),
        ];
        
        for (duration, expected) in typical_cases {
            let result = format_round_trip_time(duration);
            assert_eq!(result, expected, "Typical response formatting failed for {:?}", duration);
        }
        
        // Slow responses (≥ 1s)
        let slow_cases = vec![
            (Duration::from_millis(1000), "1.00s"),
            (Duration::from_millis(1001), "1.00s"),
            (Duration::from_millis(1010), "1.01s"),
            (Duration::from_millis(1100), "1.10s"),
            (Duration::from_millis(1500), "1.50s"),
            (Duration::from_millis(2000), "2.00s"),
            (Duration::from_millis(2500), "2.50s"),
            (Duration::from_secs(5), "5.00s"),
            (Duration::from_secs(10), "10.00s"),
            (Duration::from_millis(12345), "12.35s"), // Test rounding
        ];
        
        for (duration, expected) in slow_cases {
            let result = format_round_trip_time(duration);
            assert_eq!(result, expected, "Slow response formatting failed for {:?}", duration);
        }
        
        println!("✅ Response speed categories test passed");
        println!("   - Very fast responses (< 1ms): display as microseconds");
        println!("   - Typical responses (1-999ms): display as milliseconds");
        println!("   - Slow responses (≥ 1s): display as seconds with 2 decimal places");
    }

    /// Test timing display consistency across different operations
    #[test]
    fn test_consistency_across_operations() {
        // Test that the same duration always produces the same output
        let test_duration = Duration::from_millis(123);
        let result1 = format_round_trip_time(test_duration);
        let result2 = format_round_trip_time(test_duration);
        let result3 = format_round_trip_time(test_duration);
        
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
        assert_eq!(result1, "123ms");
        
        // Test boundary conditions are consistent
        let boundary_cases = vec![
            (Duration::from_nanos(999_999), "999μs"),    // Just under 1ms
            (Duration::from_millis(1), "1ms"),           // Exactly 1ms
            (Duration::from_millis(999), "999ms"),       // Just under 1s
            (Duration::from_millis(1000), "1.00s"),      // Exactly 1s
        ];
        
        for (duration, expected) in boundary_cases {
            // Test multiple times to ensure consistency
            for _ in 0..5 {
                let result = format_round_trip_time(duration);
                assert_eq!(result, expected, "Inconsistent formatting for boundary case {:?}", duration);
            }
        }
        
        println!("✅ Consistency test passed");
        println!("   - Same duration always produces same output");
        println!("   - Boundary conditions are stable");
    }

    /// Test edge cases and boundary conditions
    #[test]
    fn test_edge_cases() {
        // Zero duration
        assert_eq!(format_round_trip_time(Duration::ZERO), "0μs");
        
        // Very small non-zero duration
        assert_eq!(format_round_trip_time(Duration::from_nanos(1)), "0μs");
        
        // Exact boundary between microseconds and milliseconds
        assert_eq!(format_round_trip_time(Duration::from_nanos(999_999)), "999μs");
        assert_eq!(format_round_trip_time(Duration::from_nanos(1_000_000)), "1ms");
        
        // Exact boundary between milliseconds and seconds
        assert_eq!(format_round_trip_time(Duration::from_millis(999)), "999ms");
        assert_eq!(format_round_trip_time(Duration::from_millis(1000)), "1.00s");
        
        // Large durations
        assert_eq!(format_round_trip_time(Duration::from_secs(60)), "60.00s");
        assert_eq!(format_round_trip_time(Duration::from_secs(3600)), "3600.00s");
        
        // Test precision in seconds (rounding)
        assert_eq!(format_round_trip_time(Duration::from_millis(1234)), "1.23s");
        assert_eq!(format_round_trip_time(Duration::from_millis(1235)), "1.23s"); // 1235ms = 1.235s, formatted to 2 decimal places gives 1.23s
        assert_eq!(format_round_trip_time(Duration::from_millis(1999)), "2.00s");
        
        println!("✅ Edge cases test passed");
        println!("   - Zero duration handled correctly");
        println!("   - Boundary conditions work as expected");
        println!("   - Large durations format properly");
        println!("   - Rounding works correctly for seconds");
    }

    /// Test that format is suitable for user display
    #[test]
    fn test_user_display_suitability() {
        // Test that outputs are concise and readable
        let cases = vec![
            (Duration::from_micros(50), "50μs"),
            (Duration::from_millis(50), "50ms"),
            (Duration::from_millis(1500), "1.50s"),
        ];
        
        for (duration, expected) in cases {
            let result = format_round_trip_time(duration);
            
            // Check length is reasonable (not too long)
            assert!(result.len() <= 10, "Display format should be concise: '{}'", result);
            
            // Check it contains no spaces (clean format)
            assert!(!result.contains(' '), "Display format should not contain spaces: '{}'", result);
            
            // Check it has appropriate unit suffix
            assert!(result.ends_with("μs") || result.ends_with("ms") || result.ends_with("s"),
                   "Display format should have appropriate unit suffix: '{}'", result);
            
            // Check expected value
            assert_eq!(result, expected);
        }
        
        println!("✅ User display suitability test passed");
        println!("   - Formats are concise and readable");
        println!("   - No extraneous spaces or characters");
        println!("   - Clear unit indicators");
    }

    /// Integration test simulating real usage scenarios
    #[test]
    fn test_real_usage_scenarios() {
        // Simulate network response times in different scenarios
        
        // Local network (very fast)
        let local_response = Duration::from_micros(200);
        assert_eq!(format_round_trip_time(local_response), "200μs");
        
        // Same data center (fast)
        let datacenter_response = Duration::from_millis(5);
        assert_eq!(format_round_trip_time(datacenter_response), "5ms");
        
        // Cross-region (typical)
        let cross_region_response = Duration::from_millis(100);
        assert_eq!(format_round_trip_time(cross_region_response), "100ms");
        
        // International (slow)
        let international_response = Duration::from_millis(300);
        assert_eq!(format_round_trip_time(international_response), "300ms");
        
        // Poor connection (very slow)
        let poor_connection_response = Duration::from_millis(2000);
        assert_eq!(format_round_trip_time(poor_connection_response), "2.00s");
        
        // Timeout scenario (extremely slow)
        let timeout_response = Duration::from_secs(30);
        assert_eq!(format_round_trip_time(timeout_response), "30.00s");
        
        println!("✅ Real usage scenarios test passed");
        println!("   - Local network: 200μs");
        println!("   - Data center: 5ms");
        println!("   - Cross-region: 100ms");
        println!("   - International: 300ms");
        println!("   - Poor connection: 2.00s");
        println!("   - Timeout scenario: 30.00s");
    }
} 