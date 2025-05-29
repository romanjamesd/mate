//! Wire protocol unit tests - organized by functionality
//! 
//! This test crate runs the refactored wire protocol tests from the organized
//! unit test modules instead of the massive wire_protocol_tests.rs file.

// Import common test utilities
mod common;
mod unit;

// Re-export for easier access
pub use unit::messages::wire::*; 