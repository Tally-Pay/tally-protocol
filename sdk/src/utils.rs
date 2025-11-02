//! General utility functions for Solana operations and data formatting
//!
//! This module provides commonly used utility functions for working with
//! Solana data types, currency conversions, time formatting, and other
//! helper functions used across the Tally ecosystem.

#![forbid(unsafe_code)]

use anchor_client::solana_sdk::{pubkey::Pubkey, sysvar};
use std::str::FromStr;
// Note: system_program is deprecated but still used for compatibility
#[allow(deprecated)]
use anchor_lang::system_program;

/// Convert micro-lamports to USDC decimal amount
///
/// USDC uses 6 decimal places, so 1 USDC = 1,000,000 micro-lamports.
///
/// # Arguments
/// * `micro_lamports` - Amount in micro-lamports (6 decimal places)
///
/// # Returns
/// USDC amount as f64
///
/// # Examples
/// ```
/// use tally_sdk::utils::micro_lamports_to_usdc;
///
/// assert_eq!(micro_lamports_to_usdc(1_000_000), 1.0);
/// assert_eq!(micro_lamports_to_usdc(5_500_000), 5.5);
/// ```
#[must_use]
pub fn micro_lamports_to_usdc(micro_lamports: u64) -> f64 {
    // Note: This conversion may lose precision for very large values
    // but is acceptable for USDC amounts (max supply ~80B = 80_000_000_000_000_000 micro-lamports)
    // which is well within f64's 52-bit mantissa precision
    #[allow(clippy::cast_precision_loss)]
    {
        micro_lamports as f64 / 1_000_000.0
    }
}

/// Convert USDC decimal amount to micro-lamports
///
/// USDC uses 6 decimal places, so 1 USDC = 1,000,000 micro-lamports.
///
/// # Arguments
/// * `usdc_amount` - USDC amount as f64
///
/// # Returns
/// Amount in micro-lamports
///
/// # Examples
/// ```
/// use tally_sdk::utils::usdc_to_micro_lamports;
///
/// assert_eq!(usdc_to_micro_lamports(1.0), 1_000_000);
/// assert_eq!(usdc_to_micro_lamports(5.5), 5_500_000);
/// ```
#[must_use]
pub fn usdc_to_micro_lamports(usdc_amount: f64) -> u64 {
    // Ensure non-negative values and safe conversion
    let result = usdc_amount.max(0.0) * 1_000_000.0;
    // Round to avoid precision issues and ensure we don't exceed u64::MAX
    // Safe cast: result is clamped to non-negative values and u64::MAX range
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    {
        result.round().min(18_446_744_073_709_551_615.0) as u64
    }
}

/// Convert basis points to percentage
///
/// Basis points (bps) are a common unit for expressing fees and percentages in finance.
/// 1 basis point = 0.01% = 1/10000.
///
/// # Arguments
/// * `basis_points` - Fee or percentage in basis points
///
/// # Returns
/// Percentage value (e.g., 100 bps -> 1.0%)
///
/// # Examples
/// ```
/// use tally_sdk::utils::basis_points_to_percentage;
///
/// assert_eq!(basis_points_to_percentage(100), 1.0);   // 100 bps = 1%
/// assert_eq!(basis_points_to_percentage(50), 0.5);    // 50 bps = 0.5%
/// assert_eq!(basis_points_to_percentage(1000), 10.0); // 1000 bps = 10%
/// assert_eq!(basis_points_to_percentage(10000), 100.0); // 10000 bps = 100%
/// ```
#[must_use]
pub fn basis_points_to_percentage(basis_points: u16) -> f64 {
    f64::from(basis_points) / 100.0
}

/// Check if a pubkey is a valid Solana address
///
/// # Arguments
/// * `address` - Base58 encoded address string
///
/// # Returns
/// True if valid, false otherwise
///
/// # Examples
/// ```
/// use tally_sdk::utils::is_valid_pubkey;
///
/// assert!(is_valid_pubkey("11111111111111111111111111111112")); // System program
/// assert!(!is_valid_pubkey("invalid_address"));
/// ```
#[must_use]
pub fn is_valid_pubkey(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}

/// Get system program addresses for validation
///
/// Returns a list of well-known system program addresses that are
/// commonly used in Solana operations.
///
/// # Returns
/// Vector of system program pubkeys
#[must_use]
pub fn system_programs() -> Vec<Pubkey> {
    vec![
        system_program::ID,
        spl_token::id(),
        spl_associated_token_account::id(),
        sysvar::rent::id(),
        sysvar::clock::id(),
    ]
}

/// Format duration in seconds to human readable string
///
/// # Arguments
/// * `seconds` - Duration in seconds
///
/// # Returns
/// Human readable duration string
///
/// # Examples
/// ```
/// use tally_sdk::utils::format_duration;
///
/// assert_eq!(format_duration(30), "30s");
/// assert_eq!(format_duration(90), "1m 30s");
/// assert_eq!(format_duration(3661), "1h 1m 1s");
/// assert_eq!(format_duration(90061), "1d 1h 1m 1s");
/// ```
#[must_use]
pub fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m {secs}s")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Calculate subscription renewal timestamp
///
/// # Arguments
/// * `start_timestamp` - Subscription start time (Unix timestamp)
/// * `period_seconds` - Subscription period in seconds
/// * `periods_elapsed` - Number of periods that have elapsed
///
/// # Returns
/// Next renewal timestamp
///
/// # Examples
/// ```
/// use tally_sdk::utils::calculate_next_renewal;
///
/// // Starting at Unix timestamp 1000, with 30-day periods (2592000 seconds)
/// // After 0 periods elapsed, next renewal should be at 1000 + 2592000 = 2593000
/// let next = calculate_next_renewal(1000, 2592000, 0);
/// assert_eq!(next, 2593000);
/// ```
#[must_use]
pub fn calculate_next_renewal(
    start_timestamp: i64,
    period_seconds: u64,
    periods_elapsed: u32,
) -> i64 {
    start_timestamp.saturating_add(
        period_seconds
            .saturating_mul(u64::from(periods_elapsed.saturating_add(1)))
            .try_into()
            .unwrap_or(i64::MAX),
    )
}

/// Check if subscription is due for renewal
///
/// A subscription is due for renewal if the current time is past the renewal
/// time but still within the grace period.
///
/// # Arguments
/// * `next_renewal_timestamp` - Next renewal time (Unix timestamp)
/// * `grace_period_seconds` - Grace period in seconds
///
/// # Returns
/// True if due for renewal (including grace period)
#[must_use]
pub fn is_renewal_due(next_renewal_timestamp: i64, grace_period_seconds: u64) -> bool {
    let current_timestamp = chrono::Utc::now().timestamp();
    let grace_end =
        next_renewal_timestamp.saturating_add(grace_period_seconds.try_into().unwrap_or(i64::MAX));
    current_timestamp >= next_renewal_timestamp && current_timestamp <= grace_end
}

/// Check if subscription is overdue (past grace period)
///
/// A subscription is overdue if the current time is past both the renewal
/// time and the grace period.
///
/// # Arguments
/// * `next_renewal_timestamp` - Next renewal time (Unix timestamp)
/// * `grace_period_seconds` - Grace period in seconds
///
/// # Returns
/// True if overdue (past grace period)
#[must_use]
pub fn is_subscription_overdue(next_renewal_timestamp: i64, grace_period_seconds: u64) -> bool {
    let current_timestamp = chrono::Utc::now().timestamp();
    let grace_end =
        next_renewal_timestamp.saturating_add(grace_period_seconds.try_into().unwrap_or(i64::MAX));
    current_timestamp > grace_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_lamports_to_usdc() {
        const EPSILON: f64 = 1e-10;
        assert!((micro_lamports_to_usdc(1_000_000) - 1.0).abs() < EPSILON);
        assert!((micro_lamports_to_usdc(5_500_000) - 5.5).abs() < EPSILON);
        assert!((micro_lamports_to_usdc(0) - 0.0).abs() < EPSILON);
        assert!((micro_lamports_to_usdc(500_000) - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_usdc_to_micro_lamports() {
        assert_eq!(usdc_to_micro_lamports(1.0), 1_000_000);
        assert_eq!(usdc_to_micro_lamports(5.5), 5_500_000);
        assert_eq!(usdc_to_micro_lamports(0.0), 0);
        assert_eq!(usdc_to_micro_lamports(0.5), 500_000);

        // Test negative values are clamped to 0
        assert_eq!(usdc_to_micro_lamports(-1.0), 0);
    }

    #[test]
    fn test_is_valid_pubkey() {
        // Valid system program address
        assert!(is_valid_pubkey("11111111111111111111111111111112"));

        // Invalid addresses
        assert!(!is_valid_pubkey("invalid_address"));
        assert!(!is_valid_pubkey(""));
        assert!(!is_valid_pubkey("too_short"));
    }

    #[test]
    fn test_system_programs() {
        let programs = system_programs();
        assert!(!programs.is_empty());
        assert!(programs.contains(&system_program::ID));
        assert!(programs.contains(&spl_token::id()));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(90061), "1d 1h 1m 1s");

        // Edge cases
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(3600), "1h 0m 0s");
        assert_eq!(format_duration(86400), "1d 0h 0m 0s");
    }

    #[test]
    fn test_calculate_next_renewal() {
        let start = 1000_i64;
        let period = 2_592_000_u64; // 30 days in seconds

        // First renewal (0 periods elapsed)
        assert_eq!(
            calculate_next_renewal(start, period, 0),
            start + i64::try_from(period).unwrap()
        );

        // Second renewal (1 period elapsed)
        assert_eq!(
            calculate_next_renewal(start, period, 1),
            start + i64::try_from(2 * period).unwrap()
        );
    }

    #[test]
    fn test_is_renewal_due() {
        let now = chrono::Utc::now().timestamp();
        let grace_period = 86400; // 1 day

        // Past due but within grace period
        let past_renewal = now - 3600; // 1 hour ago
        assert!(is_renewal_due(past_renewal, grace_period));

        // Future renewal
        let future_renewal = now + 3600; // 1 hour from now
        assert!(!is_renewal_due(future_renewal, grace_period));
    }

    #[test]
    fn test_is_subscription_overdue() {
        let now = chrono::Utc::now().timestamp();
        let grace_period = 86400; // 1 day

        // Way past due (beyond grace period)
        let way_past_renewal = now - (2 * 86400); // 2 days ago
        assert!(is_subscription_overdue(way_past_renewal, grace_period));

        // Within grace period
        let recent_past_renewal = now - 3600; // 1 hour ago
        assert!(!is_subscription_overdue(recent_past_renewal, grace_period));
    }
}
