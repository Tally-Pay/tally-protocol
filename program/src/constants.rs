//! Program constants
//!
//! Mathematical and protocol constants used throughout the subscription program.
//! These values are immutable and represent universal constants or protocol-level
//! invariants that should never change post-deployment.

/// Basis points divisor for percentage calculations
///
/// Basis points are a unit of measure for percentages, where 1 basis point = 0.01%.
/// This constant represents 10,000 basis points = 100%, used for fee calculations.
///
/// # Examples
/// ```ignore
/// // Calculate 2.5% fee (250 basis points):
/// let fee_bps: u16 = 250;
/// let amount: u64 = 1_000_000;
/// let fee = (amount as u128 * fee_bps as u128) / FEE_BASIS_POINTS_DIVISOR;
/// // fee = 25_000 (2.5% of 1_000_000)
/// ```
///
/// # Immutability Rationale
/// This value must remain constant because:
/// - It's a mathematical standard (10,000 bp = 100%)
/// - Changing it would break all existing fee calculations
/// - All smart contracts using basis points assume this divisor
/// - Historical transactions and accounting depend on this value
pub const FEE_BASIS_POINTS_DIVISOR: u128 = 10_000;
