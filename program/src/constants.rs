//! Program constants
//!
//! Mathematical and protocol constants used throughout the recurring payments program.
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

/// Absolute minimum subscription period in seconds (24 hours)
///
/// This constant enforces a platform-wide minimum subscription period to prevent
/// spam attacks and ensure reasonable billing cycles. Any configuration that
/// attempts to set `min_period_seconds` below this value will be rejected.
///
/// # Security Rationale (M-4)
///
/// Without this absolute minimum, a malicious actor could:
/// - Set `min_period_seconds = 0` in config initialization
/// - Create plans with 1-second billing cycles
/// - Spam the network with excessive renewal transactions
/// - Cause denial-of-service through transaction flooding
/// - Create unreasonable merchant operational burden
///
/// # Value: 86400 seconds (24 hours)
///
/// This minimum represents one day and ensures:
/// - Reasonable billing cycles for subscription services
/// - Protection against spam and abuse
/// - Practical operational overhead for merchants
/// - Alignment with industry-standard subscription practices
///
/// # Usage
///
/// This constant is validated during `init_config` to ensure all configurations
/// respect the absolute minimum period:
///
/// ```ignore
/// require!(
///     args.min_period_seconds >= ABSOLUTE_MIN_PERIOD_SECONDS,
///     SubscriptionError::InvalidConfiguration
/// );
/// ```
///
/// # Immutability Rationale
///
/// This value is a security-critical constant that should never change:
/// - Lowering it would reintroduce the spam attack vulnerability
/// - Raising it would invalidate existing configurations and plans
/// - It represents a fundamental security and usability constraint
/// - All deployments assume this minimum for spam protection
pub const ABSOLUTE_MIN_PERIOD_SECONDS: u64 = 86400;

/// Maximum plan price limit in USDC (with 6 decimals)
///
/// This constant establishes an upper bound for subscription plan pricing to prevent
/// social engineering attacks where merchants create plans with extreme prices
/// (e.g., `u64::MAX`) that could mislead subscribers.
///
/// # Value
/// 1,000,000 USDC = `1_000_000_000_000` microlamports (with 6 decimals)
///
/// # Security Rationale (M-5)
/// Without a maximum price limit, malicious or compromised merchants could:
/// - Create plans with prices near `u64::MAX` (~18.4 quintillion USDC)
/// - Exploit social engineering to trick users into approving transactions
/// - Cause UI/UX confusion with unrealistic price displays
/// - Enable potential overflow scenarios in downstream calculations
///
/// This limit provides a reasonable ceiling for subscription services while
/// preventing extreme values that have no legitimate use case.
///
/// # Validation
/// All plan creation operations must validate: `price_usdc <= MAX_PLAN_PRICE_USDC`
///
/// # Examples
/// ```ignore
/// // Valid prices (pass validation)
/// let monthly_saas = 10_000_000; // $10 USDC
/// let enterprise_plan = 100_000_000_000; // $100,000 USDC
///
/// // Invalid price (exceeds limit, fails validation)
/// let extreme_price = 2_000_000_000_000; // $2,000,000 USDC (exceeds limit)
/// ```
///
/// # Immutability Rationale
/// This value should remain constant to ensure:
/// - Consistent security boundaries across all plan creation operations
/// - Predictable validation behavior for merchants and subscribers
/// - Protection against extreme price manipulation attacks
pub const MAX_PLAN_PRICE_USDC: u64 = 1_000_000_000_000; // 1 million USDC

/// Platform base fee for recurring payments (in basis points)
///
/// This fee applies to every payment execution and is designed to be low enough
/// for hierarchical payment structures (company → dept → employee → vendor)
/// while generating sustainable protocol revenue through volume.
///
/// # Fee Economics
///
/// - Single payment: 0.25%
/// - 3-level hierarchy: 0.75% total (3 × 0.25%)
/// - 4-level hierarchy: 1.00% total (4 × 0.25%)
///
/// # Revenue Projections
///
/// - $10M monthly volume: $25K protocol revenue
/// - $100M monthly volume: $250K protocol revenue
/// - $1B monthly volume: $2.5M protocol revenue
///
/// # Immutability Rationale
///
/// This base fee is hardcoded to ensure predictable economics:
/// - Changing it would affect all payees and payers
/// - Volume-based discounts are handled via `VolumeTier`
/// - Extensions can add their own fees on top
/// - Lower than traditional payment processors (2-3%)
///
/// # Value: 25 basis points = 0.25%
pub const PLATFORM_BASE_FEE_BPS: u16 = 25;

/// Keeper fee for executing scheduled payments (in basis points)
///
/// Reduced from traditional 0.5% to support hierarchical payment architectures.
/// Combined with platform fee (0.25%), total per-transaction overhead is 0.40%.
///
/// # Hierarchical Economics
///
/// - Single payment: 0.40% total (0.25% platform + 0.15% keeper)
/// - 3-level hierarchy: 1.20% total (3 × 0.40%)
/// - 4-level hierarchy: 1.60% total (4 × 0.40%)
///
/// # Immutability Rationale
///
/// This fee incentivizes keepers to execute payments while remaining low enough
/// for multi-level hierarchical structures:
/// - Keepers receive this fee for every renewal they execute
/// - Lower than typical subscription platform keeper fees (0.5-1%)
/// - Enables viable hierarchical payment patterns
///
/// # Alternative: Flat Fee
///
/// Consider using a flat USDC fee instead of percentage for high-value
/// payments. See `KEEPER_FEE_FLAT_USDC` alternative (currently commented out).
///
/// # Value: 15 basis points = 0.15%
pub const KEEPER_FEE_BPS: u16 = 15;

/// Maximum platform fee across all volume tiers (in basis points)
///
/// Even with volume discounts, platform fee cannot exceed this maximum.
/// Prevents misconfiguration and ensures fee transparency.
///
/// # Immutability Rationale
///
/// This ceiling protects users from excessive fees:
/// - Prevents accidental or malicious fee increases
/// - Guarantees users won't pay more than 0.5% platform fee
/// - Extensions can add fees on top, but core is bounded
///
/// # Value: 50 basis points = 0.5%
pub const MAX_PLATFORM_FEE_BPS: u16 = 50;

/// Minimum platform fee across all volume tiers (in basis points)
///
/// Even at highest volume tier, platform fee cannot go below this minimum.
/// Ensures protocol sustainability at scale.
///
/// # Immutability Rationale
///
/// This floor ensures protocol viability:
/// - Prevents race-to-zero fee competition
/// - Guarantees minimum revenue for protocol maintenance
/// - High-volume users still get significant discounts (0.1% vs 0.25%)
///
/// # Value: 10 basis points = 0.1%
pub const MIN_PLATFORM_FEE_BPS: u16 = 10;

/// Volume threshold for Growth tier (in USDC microlamports with 6 decimals)
///
/// Payees who process at least this much volume in a 30-day rolling window
/// are automatically upgraded from Standard (0.25%) to Growth (0.20%) tier.
///
/// # Value: $10,000 USDC = 10,000,000,000 microlamports
pub const GROWTH_TIER_THRESHOLD_USDC: u64 = 10_000_000_000;

/// Volume threshold for Scale tier (in USDC microlamports with 6 decimals)
///
/// Payees who process at least this much volume in a 30-day rolling window
/// are automatically upgraded from Growth (0.20%) to Scale (0.15%) tier.
///
/// # Value: $100,000 USDC = 100,000,000,000 microlamports
pub const SCALE_TIER_THRESHOLD_USDC: u64 = 100_000_000_000;

/// Rolling window period for volume calculations (in seconds)
///
/// Volume is tracked over a 30-day rolling window. After this period without
/// payments, volume resets and tier returns to Standard.
///
/// # Value: 2,592,000 seconds = 30 days
pub const VOLUME_WINDOW_SECONDS: i64 = 2_592_000;
