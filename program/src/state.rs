use anchor_lang::prelude::*;

/// Merchant tier determines platform fee rate
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum MerchantTier {
    /// Free tier: 2.0% platform fee (200 basis points)
    Free,
    /// Pro tier: 1.5% platform fee (150 basis points)
    Pro,
    /// Enterprise tier: 1.0% platform fee (100 basis points)
    Enterprise,
}

impl MerchantTier {
    /// Returns the platform fee in basis points for this tier
    #[must_use]
    pub const fn fee_bps(self) -> u16 {
        match self {
            Self::Free => 200,       // 2.0%
            Self::Pro => 150,        // 1.5%
            Self::Enterprise => 100, // 1.0%
        }
    }
}

/// Merchant account stores merchant configuration and settings
/// PDA seeds: ["merchant", authority]
#[account]
#[derive(InitSpace)]
pub struct Merchant {
    /// Merchant authority (signer for merchant operations)
    pub authority: Pubkey, // 32 bytes
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey, // 32 bytes
    /// Merchant's USDC treasury ATA (where merchant fees are sent)
    pub treasury_ata: Pubkey, // 32 bytes
    /// Platform fee in basis points (0-1000, representing 0-10%)
    pub platform_fee_bps: u16, // 2 bytes
    /// Merchant tier (Free, Pro, Enterprise)
    pub tier: MerchantTier, // 1 byte (enum discriminant)
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
    /// Total space: 8 (discriminator) + 32 + 32 + 32 + 2 + 1 + 1 = 108 bytes
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
