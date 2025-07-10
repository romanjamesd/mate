use regex::Regex;

/// Helper functions for robust test verification that don't depend on specific log strings
/// Verify that a message exchange occurred by checking for the expected outcomes
pub fn verify_message_exchange_occurred(output: &str, expected_message: &str) -> bool {
    // A successful message exchange will have:
    // 1. The message content in an echo response
    // 2. Round-trip timing measurement
    // 3. Success indicators

    let has_echo_with_content =
        output.contains("Received echo") && output.contains(expected_message);
    let has_timing = output.contains("round-trip");

    has_echo_with_content && has_timing
}

/// Verify that no USER messages were sent (for command tests)
/// Note: Handshake messages are expected and don't count as user messages
pub fn verify_no_user_message_sent(output: &str) -> bool {
    // No USER message sent means:
    // 1. No echo responses (echoes only happen for user messages)
    // 2. No user round-trip timing (format: "round-trip: Xms)" not handshake timing)
    // 3. Session summary shows "No messages sent" or "Messages sent: 0"

    let has_user_echo = output.contains("Received echo:");
    let has_user_round_trip = output.contains("round-trip:") && output.contains("ms)");
    let session_shows_no_messages =
        output.contains("No messages sent") || output.contains("Messages sent: 0");

    !has_user_echo && !has_user_round_trip && session_shows_no_messages
}

/// Legacy function - prefer verify_no_user_message_sent for command tests
pub fn verify_no_message_sent(output: &str) -> bool {
    verify_no_user_message_sent(output)
}

/// Extract message count from session summary (more reliable than log parsing)
pub fn extract_message_count_from_summary(output: &str) -> Option<u32> {
    // Look for "Messages sent: X" in session summary
    let re = Regex::new(r"Messages sent: (\d+)").ok()?;
    let captures = re.captures(output)?;
    captures.get(1)?.as_str().parse().ok()
}

/// Count actual echo responses (behavioral verification)
pub fn count_echo_responses(output: &str) -> usize {
    output.matches("Received echo").count()
}

/// Count round-trip measurements (indicates messages were actually sent)
pub fn count_round_trip_measurements(output: &str) -> usize {
    output.matches("round-trip").count()
}

/// Verify session completed successfully with proper cleanup
pub fn verify_session_completed_successfully(output: &str, exit_status: bool) -> bool {
    exit_status
        && (output.contains("Session Summary") || output.contains("Goodbye"))
        && !output.contains("error")
        && !output.contains("failed")
}

/// Extract average round-trip time if available
pub fn extract_average_round_trip(output: &str) -> Option<String> {
    let re = Regex::new(r"Average round-trip time: ([0-9.]+(?:ms|µs|us))").ok()?;
    let captures = re.captures(output)?;
    Some(captures.get(1)?.as_str().to_string())
}

/// Verify that timing information is properly formatted
pub fn verify_timing_format(output: &str) -> bool {
    let timing_patterns = [
        r"round-trip: \d+µs",     // microseconds
        r"round-trip: \d+ms",     // milliseconds
        r"round-trip: \d+\.\d+s", // seconds with decimals
    ];

    timing_patterns
        .iter()
        .any(|pattern| Regex::new(pattern).is_ok_and(|re| re.is_match(output)))
}

/// Verify command execution without message sending (for help/info commands)
pub fn verify_local_command_execution(output: &str, command_indicator: &str) -> bool {
    // Command was executed if we see the expected output
    // AND no USER messages were sent to peer (handshake is OK)
    output.contains(command_indicator) && verify_no_user_message_sent(output)
}

/// Helper to validate the complete message exchange workflow
pub struct MessageExchangeVerifier {
    pub expected_messages: Vec<String>,
    pub allow_extra_system_messages: bool,
}

impl MessageExchangeVerifier {
    pub fn new(expected_messages: Vec<String>) -> Self {
        Self {
            expected_messages,
            allow_extra_system_messages: true,
        }
    }

    /// Verify all expected messages were exchanged properly
    pub fn verify(&self, output: &str) -> Result<(), String> {
        let echo_count = count_echo_responses(output);
        let timing_count = count_round_trip_measurements(output);

        // Each user message should have an echo and timing
        if echo_count < self.expected_messages.len() {
            return Err(format!(
                "Expected {} echo responses, got {}. Messages may not have been sent.",
                self.expected_messages.len(),
                echo_count
            ));
        }

        if timing_count < self.expected_messages.len() {
            return Err(format!(
                "Expected {} timing measurements, got {}. Round-trip timing missing.",
                self.expected_messages.len(),
                timing_count
            ));
        }

        // Verify each expected message appears in echo responses
        for message in &self.expected_messages {
            if !output.contains(message) {
                return Err(format!(
                    "Expected message '{}' not found in echo responses",
                    message
                ));
            }
        }

        // Check session summary if available
        if let Some(summary_count) = extract_message_count_from_summary(output) {
            let expected_total = self.expected_messages.len() as u32;
            if self.allow_extra_system_messages {
                if summary_count < expected_total {
                    return Err(format!(
                        "Session summary shows {} messages, expected at least {}",
                        summary_count, expected_total
                    ));
                }
            } else if summary_count != expected_total {
                return Err(format!(
                    "Session summary shows {} messages, expected exactly {}",
                    summary_count, expected_total
                ));
            }
        }

        Ok(())
    }
}
