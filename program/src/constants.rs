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
