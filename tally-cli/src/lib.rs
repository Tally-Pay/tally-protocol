//! Tally CLI library
//!
//! This module exports the CLI functionality for testing and potential library use.

#![forbid(unsafe_code)]

pub mod commands;
pub mod config;
pub mod utils;

// Re-export for easy access
pub use commands::*;
pub use config::*;
