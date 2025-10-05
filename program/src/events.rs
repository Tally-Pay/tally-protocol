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
    /// The keeper (transaction caller) who executed the renewal
    pub keeper: Pubkey,
    /// The fee paid to the keeper (in USDC micro-units)
    pub keeper_fee: u64,
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

/// Event emitted when a delegate mismatch is detected during subscription renewal
///
/// This warning event alerts off-chain systems and users when the token account's
/// current delegate does not match the expected merchant-specific delegate PDA.
///
/// **Root Cause**: SPL Token accounts support only ONE delegate at a time. When users
/// have subscriptions with multiple merchants, starting or canceling a subscription
/// with one merchant will overwrite or revoke the delegate for ALL other merchants.
///
/// This addresses audit finding M-3, documenting the fundamental architectural limitation
/// of SPL Token's single-delegate design. This is NOT a bug that can be fixed without
/// migrating to Token-2022 or implementing a global delegate architecture.
///
/// **Scenarios that trigger this event:**
/// 1. User subscribes to Merchant A, then Merchant B → A's delegate is overwritten
/// 2. User cancels subscription with Merchant B → All delegates are revoked
/// 3. User manually revokes delegate → All merchant subscriptions become non-functional
///
/// Off-chain systems should monitor this event to:
/// - Alert users that their subscription is non-functional due to delegate mismatch
/// - Recommend reactivating the subscription (resets delegate correctly)
/// - Suggest using per-merchant token accounts as a workaround
/// - Display clear warnings about SPL Token single-delegate limitation
/// - Reference `docs/MULTI_MERCHANT_LIMITATION.md` for detailed explanation
///
/// **Important**: This subscription will NOT renew until the user reactivates it,
/// which will reset the delegate approval correctly.
#[event]
pub struct DelegateMismatchWarning {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan with delegate mismatch
    pub plan: Pubkey,
    /// The subscriber whose token account has incorrect delegate
    pub subscriber: Pubkey,
    /// The expected delegate PDA for this merchant
    pub expected_delegate: Pubkey,
    /// The actual delegate currently set on the token account (may be None or different merchant)
    pub actual_delegate: Option<Pubkey>,
}

/// Event emitted when global configuration is updated
///
/// This event provides transparency for all configuration changes made by the platform authority.
/// Off-chain systems can monitor this event to track configuration updates and adjust behavior accordingly.
#[event]
pub struct ConfigUpdated {
    /// Keeper fee in basis points (e.g., 25 = 0.25%)
    pub keeper_fee_bps: u16,
    /// Maximum withdrawal amount per transaction in USDC microlamports
    pub max_withdrawal_amount: u64,
    /// Maximum grace period in seconds
    pub max_grace_period_seconds: u64,
    /// Minimum platform fee in basis points
    pub min_platform_fee_bps: u16,
    /// Maximum platform fee in basis points
    pub max_platform_fee_bps: u16,
    /// Platform authority who made the update
    pub updated_by: Pubkey,
}

/// Event emitted when a merchant's tier is changed
///
/// This event provides transparency and auditability for merchant tier changes.
/// Tier changes immediately affect the platform fee rate applied to new renewals.
/// Off-chain systems can monitor this event to:
/// - Track merchant tier progression and revenue impact
/// - Generate analytics on tier adoption patterns
/// - Alert merchants of tier changes initiated by platform authority
/// - Maintain audit trails for billing and compliance
#[event]
pub struct MerchantTierChanged {
    /// The merchant account whose tier changed
    pub merchant: Pubkey,
    /// The previous tier before the change
    pub old_tier: crate::state::MerchantTier,
    /// The new tier after the change
    pub new_tier: crate::state::MerchantTier,
    /// The new platform fee in basis points corresponding to the new tier
    pub new_fee_bps: u16,
}

/// Event emitted when a plan's pricing or terms are updated
///
/// This event provides transparency for all plan term modifications made by merchant authority.
/// Term updates affect existing subscriptions starting from their next renewal.
/// Off-chain systems can monitor this event to:
/// - Track pricing changes and revenue impacts
/// - Alert subscribers of upcoming term changes
/// - Generate analytics on plan evolution patterns
/// - Maintain audit trails for subscription management
#[event]
pub struct PlanTermsUpdated {
    /// The plan account whose terms were updated
    pub plan: Pubkey,
    /// The merchant who owns the plan
    pub merchant: Pubkey,
    /// The old price before update (if price was updated)
    pub old_price: Option<u64>,
    /// The new price after update (if price was updated)
    pub new_price: Option<u64>,
    /// The old period before update (if period was updated)
    pub old_period: Option<u64>,
    /// The new period after update (if period was updated)
    pub new_period: Option<u64>,
    /// The old grace period before update (if grace was updated)
    pub old_grace: Option<u64>,
    /// The new grace period after update (if grace was updated)
    pub new_grace: Option<u64>,
    /// Merchant authority who performed the update
    pub updated_by: Pubkey,
}

/// Event emitted when a subscription starts with a free trial period
///
/// This event indicates a new subscription was created with a trial period,
/// during which no payment is required. The first payment will occur when
/// the trial ends.
///
/// Off-chain systems can monitor this event to:
/// - Track trial usage and conversion rates
/// - Send trial expiration reminders to subscribers
/// - Generate analytics on trial effectiveness
/// - Identify potential trial abuse patterns
#[event]
pub struct TrialStarted {
    /// The subscription account that entered trial period
    pub subscription: Pubkey,
    /// The subscriber who started the trial
    pub subscriber: Pubkey,
    /// The subscription plan for this trial
    pub plan: Pubkey,
    /// Unix timestamp when the trial period ends
    pub trial_ends_at: i64,
}

/// Event emitted when a trial subscription converts to paid
///
/// This event marks the successful conversion of a free trial subscription
/// to a paid subscription after the trial period ends. This occurs during
/// the first renewal after trial expiration.
///
/// Off-chain systems can monitor this event to:
/// - Track trial-to-paid conversion rates
/// - Measure subscription revenue attribution
/// - Generate trial effectiveness metrics
/// - Trigger post-conversion workflows (welcome emails, etc.)
#[event]
pub struct TrialConverted {
    /// The subscription account that converted from trial to paid
    pub subscription: Pubkey,
    /// The subscriber who converted to paid
    pub subscriber: Pubkey,
    /// The subscription plan that was converted
    pub plan: Pubkey,
}
