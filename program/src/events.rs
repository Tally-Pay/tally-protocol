use anchor_lang::prelude::*;

/// Event emitted when a subscription is successfully started
#[event]
pub struct Subscribed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being subscribed to
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for the subscription (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a previously canceled subscription is reactivated
#[event]
pub struct SubscriptionReactivated {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being reactivated
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for reactivation (in USDC micro-units)
    pub amount: u64,
    /// Cumulative number of renewals across all subscription sessions
    pub total_renewals: u32,
    /// Original subscription creation timestamp (preserved from first session)
    pub original_created_ts: i64,
}

/// Event emitted when a subscription is successfully renewed
#[event]
pub struct Renewed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being renewed
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for the renewal (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a subscription is canceled
#[event]
pub struct Canceled {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being canceled
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
}

/// Event emitted when a subscription account is closed and rent is reclaimed
#[event]
pub struct SubscriptionClosed {
    /// The subscription plan that was closed
    pub plan: Pubkey,
    /// The subscriber's public key who closed the subscription and received the rent
    pub subscriber: Pubkey,
}

/// Event emitted when a subscription payment fails
#[event]
pub struct PaymentFailed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan where payment failed
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The reason for payment failure (encoded as string for off-chain analysis)
    pub reason: String,
}

/// Event emitted when a plan's active status is changed
#[event]
pub struct PlanStatusChanged {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan whose status changed
    pub plan: Pubkey,
    /// The new active status
    pub active: bool,
    /// Who changed the status: "merchant" or "platform"
    pub changed_by: String,
}

/// Event emitted when global configuration is initialized
#[event]
pub struct ConfigInitialized {
    /// Platform authority pubkey for admin operations
    pub platform_authority: Pubkey,
    /// Maximum platform fee in basis points
    pub max_platform_fee_bps: u16,
    /// Minimum platform fee in basis points
    pub min_platform_fee_bps: u16,
    /// Minimum subscription period in seconds
    pub min_period_seconds: u64,
    /// Default allowance periods multiplier
    pub default_allowance_periods: u8,
    /// Allowed token mint address (e.g., official USDC mint)
    pub allowed_mint: Pubkey,
    /// Maximum withdrawal amount per transaction in USDC microlamports
    pub max_withdrawal_amount: u64,
    /// Maximum grace period in seconds
    pub max_grace_period_seconds: u64,
    /// Unix timestamp when config was initialized
    pub timestamp: i64,
}

/// Event emitted when a merchant account is initialized
#[event]
pub struct MerchantInitialized {
    /// The merchant PDA account
    pub merchant: Pubkey,
    /// Merchant authority (signer for merchant operations)
    pub authority: Pubkey,
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey,
    /// Merchant's USDC treasury ATA
    pub treasury_ata: Pubkey,
    /// Platform fee in basis points
    pub platform_fee_bps: u16,
    /// Unix timestamp when merchant was initialized
    pub timestamp: i64,
}

/// Event emitted when a subscription plan is created
#[event]
pub struct PlanCreated {
    /// The plan PDA account
    pub plan: Pubkey,
    /// Reference to the merchant PDA
    pub merchant: Pubkey,
    /// Deterministic plan identifier
    pub plan_id: String,
    /// Price in USDC microlamports (6 decimals)
    pub price_usdc: u64,
    /// Subscription period in seconds
    pub period_secs: u64,
    /// Grace period for renewals in seconds
    pub grace_secs: u64,
    /// Plan display name
    pub name: String,
    /// Unix timestamp when plan was created
    pub timestamp: i64,
}

/// Event emitted when the program is paused
#[event]
pub struct ProgramPaused {
    /// Platform authority who initiated the pause
    pub authority: Pubkey,
    /// Unix timestamp when program was paused
    pub timestamp: i64,
}

/// Event emitted when the program is unpaused
#[event]
pub struct ProgramUnpaused {
    /// Platform authority who initiated the unpause
    pub authority: Pubkey,
    /// Unix timestamp when program was unpaused
    pub timestamp: i64,
}

/// Event emitted when a subscription renewal succeeds but remaining allowance is low
///
/// This warning event alerts off-chain systems and users when the delegate allowance
/// drops below a recommended threshold (2x the plan price). While the current renewal
/// succeeded, the low allowance may cause the next renewal to fail if not topped up.
///
/// This addresses the allowance management UX concern from audit finding L-3, where
/// users may successfully start a subscription with multi-period allowance but find
/// renewals failing if allowance drops below the single-period price.
///
/// Off-chain systems should monitor this event to:
/// - Send notifications to users to increase their allowance
/// - Display warnings in UI before the next renewal date
/// - Trigger automated allowance top-up workflows
/// - Generate analytics on allowance management patterns
#[event]
pub struct LowAllowanceWarning {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan with low allowance
    pub plan: Pubkey,
    /// The subscriber who needs to increase allowance
    pub subscriber: Pubkey,
    /// Current remaining allowance (in USDC micro-units)
    pub current_allowance: u64,
    /// Recommended minimum allowance (2x plan price)
    pub recommended_allowance: u64,
    /// Plan price for reference (in USDC micro-units)
    pub plan_price: u64,
}

/// Event emitted when platform fees are withdrawn
///
/// This event provides transparency and auditability for all platform fee withdrawals,
/// addressing audit finding L-8. Off-chain systems can monitor this event to:
/// - Track fee withdrawal history and patterns
/// - Generate financial reports and analytics
/// - Alert on unusual withdrawal activity
/// - Maintain audit trails for compliance
#[event]
pub struct FeesWithdrawn {
    /// Platform authority who authorized the withdrawal
    pub platform_authority: Pubkey,
    /// Destination ATA where fees were sent
    pub destination: Pubkey,
    /// Amount withdrawn in USDC micro-units
    pub amount: u64,
    /// Unix timestamp when withdrawal occurred
    pub timestamp: i64,
}
