/// CI-specific test utilities for handling backtrace behavior
///
/// This module provides helper functions to handle the differences between
/// local and CI environments, particularly around RUST_BACKTRACE behavior.
use std::env;

/// Check if we're in a CI environment with backtrace enabled
pub fn is_ci_with_backtrace() -> bool {
    let is_ci = env::var("CI").is_ok() || env::var("GITHUB_ACTIONS").is_ok();
    let has_backtrace = env::var("RUST_BACKTRACE").is_ok_and(|v| v == "1" || v == "full");
    is_ci && has_backtrace
}

/// Check if we're in a CI environment with backtrace enabled (for testing)
/// This version accepts explicit environment values to avoid global state manipulation
pub fn is_ci_with_backtrace_explicit(
    ci_set: bool,
    github_actions_set: bool,
    rust_backtrace: Option<&str>,
) -> bool {
    let is_ci = ci_set || github_actions_set;
    let has_backtrace = rust_backtrace.is_some_and(|v| v == "1" || v == "full");
    is_ci && has_backtrace
}

/// Filter backtrace-related output for CI environments
pub fn filter_ci_backtrace_output(output: &str) -> String {
    if !is_ci_with_backtrace() {
        return output.to_string();
    }

    // Remove CI-specific backtrace noise while preserving user-facing errors
    let lines: Vec<&str> = output.lines().collect();
    let mut filtered_lines = Vec::new();
    let mut in_backtrace = false;

    for line in lines {
        if line.contains("stack backtrace:") {
            in_backtrace = true;
            continue;
        }
        if in_backtrace && (line.trim().is_empty() || line.starts_with("note:")) {
            in_backtrace = false;
            continue;
        }
        if !in_backtrace {
            filtered_lines.push(line);
        }
    }

    filtered_lines.join("\n")
}

/// Check if output contains user-facing errors (excluding CI backtraces)
pub fn contains_user_facing_errors(output: &str) -> bool {
    let filtered_output = filter_ci_backtrace_output(output);

    // Check for actual user-facing errors (not CI backtraces)
    let has_user_panic =
        filtered_output.contains("panic") && !filtered_output.contains("RUST_BACKTRACE=1");

    let has_user_backtrace = filtered_output.contains("backtrace") && !is_ci_with_backtrace();

    // Check for other problematic strings that shouldn't be shown to users
    let has_thread_panic = filtered_output.contains("thread panicked");
    let has_sigabrt = filtered_output.contains("SIGABRT");
    let has_rust_backtrace = filtered_output.contains("rust backtrace") && !is_ci_with_backtrace();

    has_user_panic || has_user_backtrace || has_thread_panic || has_sigabrt || has_rust_backtrace
}

/// Validate that error output is appropriate for the current environment
pub fn validate_error_output(output: &str, test_name: &str) -> bool {
    let filtered_output = filter_ci_backtrace_output(output);

    // Check for actual user-facing errors (not CI backtraces)
    let has_user_panic =
        filtered_output.contains("panic") && !filtered_output.contains("RUST_BACKTRACE=1");

    let has_user_backtrace = filtered_output.contains("backtrace") && !is_ci_with_backtrace();

    let has_thread_panic = filtered_output.contains("thread panicked");
    let has_sigabrt = filtered_output.contains("SIGABRT");
    let has_rust_backtrace = filtered_output.contains("rust backtrace") && !is_ci_with_backtrace();

    // Fail if user-facing errors are present
    if has_user_panic || has_user_backtrace || has_thread_panic || has_sigabrt || has_rust_backtrace
    {
        eprintln!("❌ Test {} failed: User-facing errors detected", test_name);
        eprintln!("Filtered output: {}", filtered_output);
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ci_with_backtrace_detection() {
        // Test when neither CI nor RUST_BACKTRACE is set
        assert!(!is_ci_with_backtrace_explicit(false, false, None));
        assert!(!is_ci_with_backtrace_explicit(false, false, Some("")));

        // Test when CI is set but no backtrace
        assert!(!is_ci_with_backtrace_explicit(true, false, None));
        assert!(!is_ci_with_backtrace_explicit(true, false, Some("")));
        assert!(!is_ci_with_backtrace_explicit(true, false, Some("0")));

        // Test when GITHUB_ACTIONS is set but no backtrace
        assert!(!is_ci_with_backtrace_explicit(false, true, None));
        assert!(!is_ci_with_backtrace_explicit(false, true, Some("")));

        // Test when backtrace is set but no CI
        assert!(!is_ci_with_backtrace_explicit(false, false, Some("1")));
        assert!(!is_ci_with_backtrace_explicit(false, false, Some("full")));

        // Test when both CI and backtrace are set
        assert!(is_ci_with_backtrace_explicit(true, false, Some("1")));
        assert!(is_ci_with_backtrace_explicit(true, false, Some("full")));
        assert!(is_ci_with_backtrace_explicit(false, true, Some("1")));
        assert!(is_ci_with_backtrace_explicit(false, true, Some("full")));

        // Test edge cases
        assert!(!is_ci_with_backtrace_explicit(true, false, Some("true"))); // Invalid backtrace value
        assert!(!is_ci_with_backtrace_explicit(true, false, Some("yes"))); // Invalid backtrace value
    }

    #[test]
    fn test_filter_ci_backtrace_output() {
        let test_output = "Error occurred\nstack backtrace:\n   0: some_function\n   1: another_function\nnote: some note\nActual error message";

        // Create a helper function that simulates the filtering logic with explicit parameters
        let filter_with_explicit_ci = |output: &str, ci_active: bool| -> String {
            if !ci_active {
                return output.to_string();
            }

            // Apply the same filtering logic as filter_ci_backtrace_output
            let lines: Vec<&str> = output.lines().collect();
            let mut filtered_lines = Vec::new();
            let mut in_backtrace = false;

            for line in lines {
                if line.contains("stack backtrace:") {
                    in_backtrace = true;
                    continue;
                }
                if in_backtrace && (line.trim().is_empty() || line.starts_with("note:")) {
                    in_backtrace = false;
                    continue;
                }
                if !in_backtrace {
                    filtered_lines.push(line);
                }
            }

            filtered_lines.join("\n")
        };

        // When not in CI, should return original output
        let filtered = filter_with_explicit_ci(test_output, false);
        assert_eq!(filtered, test_output);

        // When in CI with backtrace, should filter out backtrace
        let filtered = filter_with_explicit_ci(test_output, true);
        assert!(!filtered.contains("stack backtrace:"));
        assert!(!filtered.contains("0: some_function"));
        assert!(filtered.contains("Error occurred"));
        assert!(filtered.contains("Actual error message"));
    }

    #[test]
    fn test_contains_user_facing_errors() {
        // Test with clean output
        let clean_output = "Command executed successfully";
        assert!(!contains_user_facing_errors(clean_output));

        // Test with user-facing panic
        let panic_output = "Error: Application panic occurred";
        assert!(contains_user_facing_errors(panic_output));

        // Test with thread panic
        let thread_panic_output = "thread panicked at 'message'";
        assert!(contains_user_facing_errors(thread_panic_output));

        // Test with SIGABRT
        let sigabrt_output = "Process terminated with SIGABRT";
        assert!(contains_user_facing_errors(sigabrt_output));
    }

    #[test]
    fn test_validate_error_output() {
        // Test with clean output
        let clean_output = "Command executed successfully";
        assert!(validate_error_output(clean_output, "test_clean"));

        // Test with user-facing errors
        let error_output = "Error: panic occurred in user code";
        assert!(!validate_error_output(error_output, "test_error"));
    }
}
