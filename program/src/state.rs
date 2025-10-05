use anchor_lang::prelude::*;

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
    /// Number of renewals processed
    pub renewals: u32, // 4 bytes
    /// Unix timestamp when subscription was created
    pub created_ts: i64, // 8 bytes
    /// Last charged amount for audit purposes
    pub last_amount: u64, // 8 bytes
    /// Unix timestamp when subscription was last renewed (prevents double-renewal attacks)
    pub last_renewed_ts: i64, // 8 bytes
    /// PDA bump seed
    pub bump: u8, // 1 byte
}

impl Merchant {
    /// Total space: 8 (discriminator) + 32 + 32 + 32 + 2 + 1 = 107 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

impl Plan {
    /// Total space: 8 (discriminator) + 32 + 32 + 8 + 8 + 8 + 32 + 1 = 129 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}

impl Subscription {
    /// Total space: 8 (discriminator) + 32 + 32 + 8 + 1 + 4 + 8 + 8 + 8 + 1 = 110 bytes
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
    /// PDA bump seed
    pub bump: u8, // 1 byte
}

impl Config {
    /// Total space: 8 (discriminator) + 32 + 33 + 2 + 2 + 8 + 1 + 32 + 1 = 119 bytes
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
}
