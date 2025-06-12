//! Test organization for the mate messaging system
//!
//! This module organizes tests into logical groupings:
//! - `common`: Shared test utilities and helpers
//! - `unit`: Unit tests for individual components
//! - `integration`: Integration tests for full system behavior
//! - `security`: Security-focused tests including DoS protection
//! - `performance`: Performance and resource usage tests
//! - `storage`: Storage layer tests for SQLite operations

pub mod common;
pub mod integration;
pub mod performance;
pub mod security;
pub mod storage;
pub mod unit;
