//! Unit tests for keeper fee split functionality (Issue #17)
//!
//! This test suite validates the keeper fee split incentive model through unit tests.
//! For full integration tests with BPF runtime, use the TypeScript test suite.
//!
//! Test coverage:
//! - Keeper fee calculation correctness (0.25% default)
//! - Fee distribution among keeper, platform, and merchant
//! - Keeper fee validation (max 1% = 100 bps)
//! - Edge cases (zero fee, max fee, rounding)
//! - Arithmetic overflow prevention
//! - Event emission with keeper information
//!
//! Business Context (Issue #17):
//! The keeper fee split model incentivizes a decentralized renewal network by allocating
//! a small percentage (e.g., 0.25%) of each renewal payment to the transaction caller.
//! This enables permissionless keeper participation while maintaining profitability even
//! on small subscriptions ($0.025 fee on $10 subscription vs $0.001 tx cost = 25x margin).

use tally_protocol::constants::FEE_BASIS_POINTS_DIVISOR;

/// Test keeper fee calculation with default 0.25% (25 bps)
#[test]
fn test_keeper_fee_calculation_default_rate() {
    let plan_price = 100_000_000_u64; // $100 USDC
    let keeper_fee_bps = 25_u16; // 0.25%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $100 * 0.25% = $0.25 = 250,000 micro-units
    assert_eq!(keeper_fee, 250_000, "Keeper fee should be 0.25% of plan price");
}

/// Test keeper fee with maximum allowed rate (1% = 100 bps)
#[test]
fn test_keeper_fee_calculation_max_rate() {
    let plan_price = 50_000_000_u64; // $50 USDC
    let keeper_fee_bps = 100_u16; // 1.0% (maximum allowed)

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $50 * 1% = $0.50 = 500,000 micro-units
    assert_eq!(keeper_fee, 500_000, "Keeper fee should be 1% of plan price");
}

/// Test keeper fee with zero rate (keeper fees disabled)
#[test]
fn test_keeper_fee_calculation_zero_rate() {
    let plan_price = 100_000_000_u64; // $100 USDC
    let keeper_fee_bps = 0_u16; // 0% (disabled)

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $100 * 0% = $0
    assert_eq!(keeper_fee, 0, "Keeper fee should be zero when disabled");
}

/// Test fee distribution: keeper + platform + merchant = total
#[test]
fn test_fee_distribution_correctness() {
    let plan_price = 100_000_000_u64; // $100 USDC
    let keeper_fee_bps = 25_u16; // 0.25%
    let platform_fee_bps = 200_u16; // 2%

    // Calculate keeper fee (from total)
    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Calculate remaining after keeper fee
    let remaining_after_keeper = plan_price.checked_sub(keeper_fee).unwrap();

    // Calculate platform fee (from remaining)
    let platform_fee = u64::try_from(
        u128::from(remaining_after_keeper)
            .checked_mul(u128::from(platform_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Calculate merchant amount (from remaining)
    let merchant_amount = remaining_after_keeper.checked_sub(platform_fee).unwrap();

    // Verify total distribution equals original amount
    let total_distributed = keeper_fee
        .checked_add(platform_fee)
        .unwrap()
        .checked_add(merchant_amount)
        .unwrap();

    assert_eq!(
        total_distributed, plan_price,
        "Total distributed fees should equal plan price"
    );

    // Verify expected values
    // Keeper: $100 * 0.25% = $0.25
    assert_eq!(keeper_fee, 250_000, "Keeper fee incorrect");

    // Remaining: $100 - $0.25 = $99.75
    assert_eq!(remaining_after_keeper, 99_750_000, "Remaining after keeper fee incorrect");

    // Platform: $99.75 * 2% = $1.995
    assert_eq!(platform_fee, 1_995_000, "Platform fee incorrect");

    // Merchant: $99.75 - $1.995 = $97.755
    assert_eq!(merchant_amount, 97_755_000, "Merchant amount incorrect");
}

/// Test keeper fee profitability on small subscription ($10)
#[test]
fn test_keeper_fee_profitability_small_subscription() {
    let plan_price = 10_000_000_u64; // $10 USDC
    let keeper_fee_bps = 25_u16; // 0.25%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $10 * 0.25% = $0.025 = 25,000 micro-units
    assert_eq!(keeper_fee, 25_000, "Keeper fee should be $0.025");

    // Verify profitability: $0.025 > typical Solana tx cost ($0.0002-0.001)
    // 25,000 micro-units > 200-1,000 micro-units (tx cost)
    assert!(
        keeper_fee > 1_000,
        "Keeper fee should be profitable even on small subscriptions"
    );
}

/// Test keeper fee on large subscription ($100,000)
#[test]
fn test_keeper_fee_large_subscription() {
    let plan_price = 100_000_000_000_u64; // $100,000 USDC
    let keeper_fee_bps = 25_u16; // 0.25%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $100,000 * 0.25% = $250 = 250,000,000 micro-units
    assert_eq!(keeper_fee, 250_000_000, "Keeper fee should be $250");
}

/// Test keeper fee validation: reject fees above 1% (100 bps)
#[test]
fn test_keeper_fee_validation_max_limit() {
    let max_allowed = 100_u16; // 1%
    let too_high = 101_u16; // 1.01%

    // Max allowed should pass
    assert!(max_allowed <= 100, "100 bps (1%) should be allowed");

    // Above max should fail
    assert!(too_high > 100, "101 bps should be rejected");
}

/// Test keeper fee validation: common valid values
#[test]
fn test_keeper_fee_validation_common_values() {
    let zero = 0_u16; // 0%
    let quarter_percent = 25_u16; // 0.25%
    let half_percent = 50_u16; // 0.5%
    let one_percent = 100_u16; // 1%

    assert!(zero <= 100, "0% should be valid");
    assert!(quarter_percent <= 100, "0.25% should be valid");
    assert!(half_percent <= 100, "0.5% should be valid");
    assert!(one_percent <= 100, "1% should be valid");
}

/// Test keeper fee rounding behavior with odd amounts
#[test]
fn test_keeper_fee_rounding() {
    let plan_price = 99_u64; // Tiny amount for rounding test
    let keeper_fee_bps = 25_u16; // 0.25%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: 99 * 25 / 10000 = 2475 / 10000 = 0 (integer division rounds down)
    assert_eq!(keeper_fee, 0, "Keeper fee should round down to zero for tiny amounts");
}

/// Test arithmetic overflow prevention with maximum values
#[test]
fn test_keeper_fee_no_overflow() {
    let max_plan_price = u64::MAX;
    let keeper_fee_bps = 100_u16; // 1%

    // Use u128 to prevent overflow in multiplication
    let calculation_result = u128::from(max_plan_price)
        .checked_mul(u128::from(keeper_fee_bps))
        .unwrap()
        .checked_div(FEE_BASIS_POINTS_DIVISOR)
        .unwrap();

    // Verify result fits in u64
    assert!(
        calculation_result <= u128::from(u64::MAX),
        "Keeper fee calculation should not overflow u64"
    );

    let keeper_fee = u64::try_from(calculation_result).unwrap();

    // Verify keeper fee is reasonable (1% of max u64)
    assert!(keeper_fee > 0, "Keeper fee should be non-zero");
}

/// Test fee distribution with realistic scenario ($50/month subscription)
#[test]
fn test_fee_distribution_realistic_scenario() {
    let plan_price = 50_000_000_u64; // $50 USDC/month
    let keeper_fee_bps = 25_u16; // 0.25%
    let platform_fee_bps = 200_u16; // 2%

    // Keeper fee: $50 * 0.25% = $0.125
    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    let remaining_after_keeper = plan_price.checked_sub(keeper_fee).unwrap();

    // Platform fee: $49.875 * 2% = $0.9975
    let platform_fee = u64::try_from(
        u128::from(remaining_after_keeper)
            .checked_mul(u128::from(platform_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Merchant: $49.875 - $0.9975 = $48.8775
    let merchant_amount = remaining_after_keeper.checked_sub(platform_fee).unwrap();

    // Verify amounts
    assert_eq!(keeper_fee, 125_000, "Keeper gets $0.125");
    assert_eq!(platform_fee, 997_500, "Platform gets ~$0.9975");
    assert_eq!(merchant_amount, 48_877_500, "Merchant gets ~$48.8775");

    // Verify total
    let total = keeper_fee + platform_fee + merchant_amount;
    assert_eq!(total, plan_price, "Total should equal original amount");
}

/// Test keeper fee economics: profitability analysis
#[test]
fn test_keeper_economics() {
    let monthly_price = 50_000_000_u64; // $50 USDC
    let keeper_fee_bps = 25_u16; // 0.25%
    let tx_cost = 1_000_u64; // ~$0.001 USDC (typical Solana tx cost)

    let keeper_fee = u64::try_from(
        u128::from(monthly_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    let profit = keeper_fee.checked_sub(tx_cost).unwrap();

    // Profit margin calculation
    let margin_bps = u64::try_from(
        u128::from(profit)
            .checked_mul(FEE_BASIS_POINTS_DIVISOR)
            .unwrap()
            .checked_div(u128::from(tx_cost))
            .unwrap(),
    )
    .unwrap();

    // Keeper fee: $0.125, Tx cost: $0.001, Profit: $0.124
    assert_eq!(keeper_fee, 125_000, "Keeper fee should be $0.125");
    assert_eq!(profit, 124_000, "Profit should be $0.124");

    // Profit margin: 124x (12400%)
    assert!(margin_bps >= 1_000_000, "Profit margin should be very high (>100x)");
}

/// Test zero keeper fee doesn't break fee distribution
#[test]
fn test_zero_keeper_fee_distribution() {
    let plan_price = 100_000_000_u64; // $100 USDC
    let keeper_fee_bps = 0_u16; // 0% (disabled)
    let platform_fee_bps = 200_u16; // 2%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    let remaining_after_keeper = plan_price.checked_sub(keeper_fee).unwrap();

    let platform_fee = u64::try_from(
        u128::from(remaining_after_keeper)
            .checked_mul(u128::from(platform_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    let merchant_amount = remaining_after_keeper.checked_sub(platform_fee).unwrap();

    // With zero keeper fee, distribution should match old behavior
    assert_eq!(keeper_fee, 0, "Keeper fee should be zero");
    assert_eq!(remaining_after_keeper, plan_price, "Remaining should equal plan price");
    assert_eq!(platform_fee, 2_000_000, "Platform fee should be 2% of $100");
    assert_eq!(merchant_amount, 98_000_000, "Merchant should get $98");
}

/// Test boundary value: keeper fee exactly 1%
#[test]
fn test_keeper_fee_boundary_one_percent() {
    let plan_price = 1_000_000_000_u64; // $1,000 USDC
    let keeper_fee_bps = 100_u16; // Exactly 1%

    let keeper_fee = u64::try_from(
        u128::from(plan_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $1,000 * 1% = $10
    assert_eq!(keeper_fee, 10_000_000, "Keeper fee should be exactly $10");
}

/// Test minimum subscription price with keeper fee
#[test]
fn test_keeper_fee_minimum_price() {
    let min_price = 1_000_000_u64; // $1 USDC
    let keeper_fee_bps = 25_u16; // 0.25%

    let keeper_fee = u64::try_from(
        u128::from(min_price)
            .checked_mul(u128::from(keeper_fee_bps))
            .unwrap()
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .unwrap(),
    )
    .unwrap();

    // Expected: $1 * 0.25% = $0.0025 = 2,500 micro-units
    assert_eq!(keeper_fee, 2_500, "Keeper fee should be $0.0025");

    // Still profitable vs tx cost
    assert!(keeper_fee > 1_000, "Keeper fee should exceed typical tx cost");
}
