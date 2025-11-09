use anchor_lang::prelude::*;

use crate::constants::{
    GROWTH_TIER_THRESHOLD_USDC, MAX_PLATFORM_FEE_BPS, MIN_PLATFORM_FEE_BPS,
    SCALE_TIER_THRESHOLD_USDC,
};

/// Volume tier determines platform fee rate based on 30-day rolling payment volume
///
/// Tiers automatically upgrade as payees process more volume, providing fee discounts
/// that enable economically viable hierarchical payment structures.
///
/// # Fee Structure
///
/// - **Standard**: 0.25% (up to $10K monthly volume)
/// - **Growth**: 0.20% ($10K - $100K monthly volume)
/// - **Scale**: 0.15% (over $100K monthly volume)
///
/// # Automatic Tier Upgrades
///
/// Tiers are recalculated on every payment execution based on the payee's rolling
/// 30-day volume. When volume crosses a threshold, the tier automatically upgrades
/// and the new fee rate applies to all future payments.
///
/// # Volume Reset
///
/// If no payments are processed for 30 days, volume resets to zero and tier returns
/// to Standard. This prevents inactive payees from maintaining high-tier discounts.
///
/// # Hierarchical Payment Economics
///
/// Volume-based discounts make hierarchical payment structures economically viable:
///
/// **3-Level Hierarchy (all Standard tier):**
/// - Total fees: 3 × 0.25% = 0.75% platform fees
/// - Plus keeper: 3 × 0.15% = 0.45% keeper fees
/// - Total overhead: 1.20% (competitive with traditional processors)
///
/// **4-Level Hierarchy (mixed tiers):**
/// - Company (Scale, $100K+): 0.15%
/// - Department (Growth, $50K): 0.20%
/// - Employee (Standard, $5K): 0.25%
/// - Vendor (Standard, $1K): 0.25%
/// - Total overhead: 0.85% + keeper fees = ~1.45%
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum VolumeTier {
    /// Standard tier: Up to $10K monthly volume, 0.25% platform fee
    Standard,
    /// Growth tier: $10K - $100K monthly volume, 0.20% platform fee
    Growth,
    /// Scale tier: Over $100K monthly volume, 0.15% platform fee
    Scale,
}

impl VolumeTier {
    /// Returns the platform fee in basis points for this tier
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let standard_fee = VolumeTier::Standard.platform_fee_bps(); // 25 (0.25%)
    /// let growth_fee = VolumeTier::Growth.platform_fee_bps();     // 20 (0.20%)
    /// let scale_fee = VolumeTier::Scale.platform_fee_bps();       // 15 (0.15%)
    /// ```
    #[must_use]
    pub const fn platform_fee_bps(self) -> u16 {
        match self {
            Self::Standard => 25, // 0.25%
            Self::Growth => 20,   // 0.20%
            Self::Scale => 15,    // 0.15%
        }
    }

    /// Determines tier based on 30-day rolling volume
    ///
    /// # Arguments
    ///
    /// * `volume_usdc` - Total USDC volume in microlamports (6 decimals) over 30 days
    ///
    /// # Returns
    ///
    /// The appropriate tier for the given volume level
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let tier1 = VolumeTier::from_monthly_volume(5_000_000_000);    // $5K -> Standard
    /// let tier2 = VolumeTier::from_monthly_volume(50_000_000_000);   // $50K -> Growth
    /// let tier3 = VolumeTier::from_monthly_volume(500_000_000_000);  // $500K -> Scale
    /// ```
    #[must_use]
    pub const fn from_monthly_volume(volume_usdc: u64) -> Self {
        if volume_usdc >= SCALE_TIER_THRESHOLD_USDC {
            Self::Scale
        } else if volume_usdc >= GROWTH_TIER_THRESHOLD_USDC {
            Self::Growth
        } else {
            Self::Standard
        }
    }

    /// Validates that the fee for this tier is within allowed bounds
    ///
    /// # Errors
    ///
    /// Returns error if tier fee exceeds `MAX_PLATFORM_FEE_BPS` or is below `MIN_PLATFORM_FEE_BPS`
    #[must_use]
    pub const fn validate_fee_bps(&self) -> u16 {
        self.platform_fee_bps()
    }

    /// Validates that the fee for this tier is within config bounds
    ///
    /// # Errors
    ///
    /// Returns error if tier fee exceeds `MAX_PLATFORM_FEE_BPS` or is below `MIN_PLATFORM_FEE_BPS`
    pub fn validate_fee(&self) -> Result<()> {
        let fee = self.platform_fee_bps();
        require!(
            (MIN_PLATFORM_FEE_BPS..=MAX_PLATFORM_FEE_BPS).contains(&fee),
            crate::errors::SubscriptionError::InvalidConfiguration
        );
        Ok(())
    }
}

/// Merchant account stores merchant configuration and settings
/// PDA seeds: ["merchant", authority]
///
/// # Volume Tracking
///
/// The Merchant account tracks rolling 30-day payment volume to automatically
/// determine the payee's fee tier. Volume resets after 30 days of inactivity.
///
/// # Account Size
///
/// Total: 122 bytes (14 bytes larger than v1.x.x due to volume tracking)
/// - Discriminator: 8 bytes
/// - `authority`: 32 bytes
/// - `usdc_mint`: 32 bytes
/// - `treasury_ata`: 32 bytes
/// - `volume_tier`: 1 byte
/// - `monthly_volume_usdc`: 8 bytes
/// - `last_volume_update_ts`: 8 bytes
/// - bump: 1 byte
///
/// Additional rent: ~0.000098 SOL (~$0.004 at $45/SOL)
#[account]
#[derive(InitSpace)]
pub struct Merchant {
    /// Merchant authority (signer for merchant operations)
    pub authority: Pubkey, // 32 bytes
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey, // 32 bytes
    /// Merchant's USDC treasury ATA (where merchant fees are sent)
    pub treasury_ata: Pubkey, // 32 bytes

    /// Current volume tier (automatically calculated from `monthly_volume_usdc`)
    ///
    /// This tier determines the platform fee rate:
    /// - Standard: 0.25% (up to $10K monthly)
    /// - Growth: 0.20% ($10K-$100K monthly)
    /// - Scale: 0.15% (>$100K monthly)
    ///
    /// The tier is recalculated on every payment execution.
    pub volume_tier: VolumeTier, // 1 byte (enum discriminant)

    /// Rolling 30-day payment volume in USDC microlamports (6 decimals)
    ///
    /// This field accumulates total payment volume processed by this merchant
    /// over a rolling 30-day window. It resets to zero if no payments are
    /// processed for 30 days.
    ///
    /// # Update Frequency
    ///
    /// Updated on every payment execution (both initial and renewals).
    ///
    /// # Tier Calculation
    ///
    /// Used to determine `volume_tier` via `VolumeTier::from_monthly_volume()`.
    pub monthly_volume_usdc: u64, // 8 bytes

    /// Unix timestamp of last volume calculation
    ///
    /// Used to determine if 30-day window has elapsed and volume should reset.
    /// Updated on every payment execution.
    pub last_volume_update_ts: i64, // 8 bytes

    /// PDA bump seed
    pub bump: u8, // 1 byte
}

/// Plan account defines subscription plan details
/// PDA seeds: ["plan", merchant, `plan_id`]
#[account]
#[derive(InitSpace)]
pub struct Plan {
    /// Reference to the merchant PDA
    pub merchant: Pubkey, // 32 bytes
    /// Deterministic plan identifier (string as bytes, padded to 32)
    pub plan_id: [u8; 32], // 32 bytes
    /// Price in USDC microlamports (6 decimals)
    pub price_usdc: u64, // 8 bytes
    /// Subscription period in seconds
    pub period_secs: u64, // 8 bytes
    /// Grace period for renewals in seconds
    pub grace_secs: u64, // 8 bytes
    /// Plan display name (string as bytes, padded to 32)
    pub name: [u8; 32], // 32 bytes
    /// Whether new subscriptions can be created for this plan
    pub active: bool, // 1 byte
}

/// Subscription account tracks individual user subscriptions
/// PDA seeds: ["subscription", plan, subscriber]
#[account]
#[derive(InitSpace)]
pub struct Subscription {
    /// Reference to the plan PDA
    pub plan: Pubkey, // 32 bytes
    /// User's pubkey (the subscriber)
    pub subscriber: Pubkey, // 32 bytes
    /// Unix timestamp for next renewal
    pub next_renewal_ts: i64, // 8 bytes
    /// Whether subscription is active
    pub active: bool, // 1 byte
    /// Number of renewals processed for this subscription.
    ///
    /// This counter increments with each successful renewal payment and is preserved
    /// across subscription cancellation and reactivation cycles. When a subscription
    /// is canceled and later reactivated, this field retains its historical value
    /// rather than resetting to zero.
    ///
    /// # Reactivation Behavior
    ///
    /// - **New Subscription**: Initialized to `0`
    /// - **Each Renewal**: Incremented by `1`
    /// - **Cancellation**: Preserved (not reset)
    /// - **Reactivation**: Preserved (continues from previous value)
    ///
    /// # Use Cases
    ///
    /// This preservation behavior is intentional to maintain a complete historical
    /// record of all renewals across the lifetime of the subscription relationship,
    /// regardless of interruptions. Off-chain systems using this field for analytics,
    /// business logic, or rewards programs must account for the possibility that this
    /// value may represent renewals from previous subscription sessions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Initial subscription
    /// subscription.renewals = 0;
    ///
    /// // After 10 renewals
    /// subscription.renewals = 10;
    ///
    /// // User cancels subscription
    /// subscription.active = false;
    /// subscription.renewals = 10; // Preserved
    ///
    /// // User reactivates subscription
    /// subscription.active = true;
    /// subscription.renewals = 10; // Still preserved, not reset to 0
    ///
    /// // After 5 more renewals in the new session
    /// subscription.renewals = 15; // Cumulative across all sessions
    /// ```
    pub renewals: u32, // 4 bytes
    /// Unix timestamp when subscription was created
    pub created_ts: i64, // 8 bytes
    /// Last charged amount for audit purposes
    pub last_amount: u64, // 8 bytes
    /// Unix timestamp when subscription was last renewed (prevents double-renewal attacks)
    pub last_renewed_ts: i64, // 8 bytes
    /// Unix timestamp when free trial period ends (None if no trial)
    ///
    /// When present, indicates the subscription is in or was in a free trial period.
    /// During the trial, no payment is required. After `trial_ends_at`, the first
    /// renewal will process the initial payment.
    ///
    /// # Trial Behavior
    ///
    /// - **New Subscription with Trial**: Set to `current_time` + `trial_duration_secs`
    /// - **During Trial**: No payment required, `in_trial` = true
    /// - **Trial End**: First renewal processes payment, `in_trial` set to false
    /// - **Reactivation**: Always None (trials only apply to first subscription)
    pub trial_ends_at: Option<i64>, // 9 bytes (1 byte discriminator + 8 bytes i64)
    /// Whether subscription is currently in free trial period
    ///
    /// When true, the subscription is active but no payment has been made yet.
    /// The first payment will occur at `next_renewal_ts` (when trial ends).
    ///
    /// # Trial State Transitions
    ///
    /// - **New Subscription**: Set to true if `trial_duration_secs` provided
    /// - **Trial End (First Renewal)**: Set to false after successful payment
    /// - **Reactivation**: Always false (no trials on reactivation)
    pub in_trial: bool, // 1 byte
    /// PDA bump seed
    pub bump: u8, // 1 byte
}

impl Merchant {
    /// Total space: 8 (discriminator) + 32 + 32 + 32 + 1 + 8 + 8 + 1 = 122 bytes
    /// Note: Previous version was 108 bytes. New version adds:
    /// - `monthly_volume_usdc`: 8 bytes
    /// - `last_volume_update_ts`: 8 bytes
    /// - Removes `platform_fee_bps`: 2 bytes (fee now derived from tier)
    ///
    ///   Net increase: 14 bytes (~0.000098 SOL additional rent)
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

impl Plan {
    /// Total space: 8 (discriminator) + 32 + 32 + 8 + 8 + 8 + 32 + 1 = 129 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

impl Subscription {
    /// Total space: 8 (discriminator) + 32 + 32 + 8 + 1 + 4 + 8 + 8 + 8 + 9 + 1 + 1 = 120 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

/// Global configuration account for program constants and settings
/// PDA seeds: `["config"]`
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Platform authority pubkey for admin operations
    pub platform_authority: Pubkey, // 32 bytes
    /// Pending authority for two-step authority transfer
    pub pending_authority: Option<Pubkey>, // 33 bytes (1 byte discriminator + 32 bytes pubkey)
    /// Maximum platform fee in basis points (e.g., 1000 = 10%)
    pub max_platform_fee_bps: u16, // 2 bytes
    /// Minimum platform fee in basis points (e.g., 50 = 0.5%)
    pub min_platform_fee_bps: u16, // 2 bytes
    /// Minimum subscription period in seconds (e.g., 86400 = 24 hours)
    pub min_period_seconds: u64, // 8 bytes
    /// Default allowance periods multiplier (e.g., 3)
    pub default_allowance_periods: u8, // 1 byte
    /// Allowed token mint address (e.g., official USDC mint)
    /// This prevents merchants from using fake or arbitrary tokens
    pub allowed_mint: Pubkey, // 32 bytes
    /// Maximum withdrawal amount per transaction in USDC microlamports
    /// Prevents accidental or malicious drainage of entire treasury
    pub max_withdrawal_amount: u64, // 8 bytes
    /// Maximum grace period in seconds (e.g., 604800 = 7 days)
    /// Prevents excessively long grace periods that increase merchant payment risk
    pub max_grace_period_seconds: u64, // 8 bytes
    /// Emergency pause state - when true, all user-facing operations are disabled
    /// This allows the platform authority to halt operations in case of security incidents
    pub paused: bool, // 1 byte
    /// Keeper fee in basis points (e.g., 25 = 0.25%)
    /// This fee is paid to the transaction caller (keeper) to incentivize decentralized renewal network
    /// Capped at 100 basis points (1%) to prevent excessive keeper fees
    pub keeper_fee_bps: u16, // 2 bytes
    /// PDA bump seed
    pub bump: u8, // 1 byte
}

impl Config {
    /// Total space: 8 (discriminator) + 32 + 33 + 2 + 2 + 8 + 1 + 32 + 8 + 8 + 1 + 2 + 1 = 138 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}
