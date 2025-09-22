//! Configuration management for the Tally CLI
//!
//! Centralizes all configuration values that were previously hardcoded,
//! making them configurable via environment variables with sensible defaults.

use std::env;

/// Centralized configuration for the Tally CLI
#[derive(Debug, Clone)]
pub struct TallyCliConfig {
    /// Default RPC URL for Solana connections
    pub default_rpc_url: String,

    /// Default output format for CLI commands
    pub default_output_format: String,

    /// USDC decimals divisor for converting micro-units to display units
    pub usdc_decimals_divisor: u64,

    /// Basis points divisor for fee calculations
    pub basis_points_divisor: f64,

    /// Default lookback time for dashboard events in seconds
    #[allow(dead_code)] // Used when dashboard functionality is re-enabled
    pub default_events_lookback_secs: i64,
}

impl TallyCliConfig {
    /// Create a new configuration instance with values from environment variables
    /// or sensible defaults if not set
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_rpc_url: env::var("TALLY_RPC_URL")
                .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string()),

            default_output_format: env::var("TALLY_DEFAULT_OUTPUT_FORMAT")
                .unwrap_or_else(|_| "human".to_string()),

            usdc_decimals_divisor: env::var("USDC_DECIMALS_DIVISOR")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_000_000),

            basis_points_divisor: env::var("BASIS_POINTS_DIVISOR")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100.0),

            default_events_lookback_secs: env::var("TALLY_DEFAULT_EVENTS_LOOKBACK_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600), // 1 hour
        }
    }

    /// Convert USDC micro-units to display units (USDC)
    #[allow(clippy::cast_precision_loss)] // Acceptable for display formatting
    #[must_use]
    pub fn format_usdc(&self, micro_units: u64) -> f64 {
        micro_units as f64 / self.usdc_decimals_divisor as f64
    }

    /// Convert fee basis points to percentage
    #[must_use]
    pub fn format_fee_percentage(&self, fee_bps: u16) -> f64 {
        f64::from(fee_bps) / self.basis_points_divisor
    }

    /// Get the default lookback timestamp for dashboard events
    #[allow(dead_code)] // Used when dashboard functionality is re-enabled
    #[must_use]
    pub const fn default_events_since_timestamp(&self, current_timestamp: i64) -> i64 {
        current_timestamp - self.default_events_lookback_secs
    }
}

impl Default for TallyCliConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = TallyCliConfig::new();

        // Test that defaults are sensible
        assert_eq!(config.default_rpc_url, "https://api.devnet.solana.com");
        assert_eq!(config.default_output_format, "human");
        assert_eq!(config.usdc_decimals_divisor, 1_000_000);
        assert!((config.basis_points_divisor - 100.0).abs() < f64::EPSILON);
        assert_eq!(config.default_events_lookback_secs, 3600);
    }

    #[test]
    fn test_usdc_formatting() {
        let config = TallyCliConfig::new();

        assert!((config.format_usdc(1_000_000) - 1.0).abs() < f64::EPSILON);
        assert!((config.format_usdc(5_000_000) - 5.0).abs() < f64::EPSILON);
        assert!((config.format_usdc(500_000) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fee_percentage_formatting() {
        let config = TallyCliConfig::new();

        assert!((config.format_fee_percentage(50) - 0.5).abs() < f64::EPSILON);
        assert!((config.format_fee_percentage(100) - 1.0).abs() < f64::EPSILON);
        assert!((config.format_fee_percentage(1000) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_events_timestamp() {
        let config = TallyCliConfig::new();
        let current = 7200; // 2 hours in seconds

        assert_eq!(config.default_events_since_timestamp(current), 3600); // 1 hour ago
    }
}
