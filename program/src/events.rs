use anchor_lang::prelude::*;

/// Event emitted when a payment agreement is successfully started
#[event]
pub struct PaymentAgreementStarted {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being agreed to
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid for the payment agreement (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a previously paused payment agreement is reactivated
#[event]
pub struct PaymentAgreementReactivated {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being reactivated
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid for reactivation (in USDC micro-units)
    pub amount: u64,
    /// Cumulative number of payments across all agreement sessions
    pub total_payments: u32,
    /// Original payment agreement creation timestamp (preserved from first session)
    pub original_created_ts: i64,
}

/// Event emitted when a recurring payment is successfully executed
#[event]
pub struct PaymentExecuted {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being executed
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid for the execution (in USDC micro-units)
    pub amount: u64,
    /// The keeper (transaction caller) who executed the payment
    pub keeper: Pubkey,
    /// The fee paid to the keeper (in USDC micro-units)
    pub keeper_fee: u64,
}

/// Event emitted when a payment agreement is paused
#[event]
pub struct PaymentAgreementPaused {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being paused
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
}

/// Event emitted when a payment agreement account is closed and rent is reclaimed
#[event]
pub struct PaymentAgreementClosed {
    /// The payment terms that was closed
    pub payment_terms: Pubkey,
    /// The payer's public key who closed the payment agreement and received the rent
    pub payer: Pubkey,
}

/// Event emitted when a recurring payment fails
#[event]
pub struct PaymentFailed {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms where payment failed
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The reason for payment failure (encoded as string for off-chain analysis)
    pub reason: String,
}

/// Event emitted when payment terms' active status is changed
#[event]
pub struct PaymentTermsStatusChanged {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms whose status changed
    pub payment_terms: Pubkey,
    /// The new active status
    pub active: bool,
    /// Who changed the status: "payee" or "platform"
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

/// Event emitted when a payee account is initialized
#[event]
pub struct PayeeInitialized {
    /// The payee PDA account
    pub payee: Pubkey,
    /// Payee authority (signer for payee operations)
    pub authority: Pubkey,
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey,
    /// Payee's USDC treasury ATA
    pub treasury_ata: Pubkey,
    /// Initial volume tier (always Standard for new payees)
    pub volume_tier: crate::state::VolumeTier,
    /// Platform fee in basis points (derived from volume tier)
    pub platform_fee_bps: u16,
    /// Unix timestamp when payee was initialized
    pub timestamp: i64,
}

/// Event emitted when payment terms are created
#[event]
pub struct PaymentTermsCreated {
    /// The payment terms PDA account
    pub payment_terms: Pubkey,
    /// Reference to the payee PDA
    pub payee: Pubkey,
    /// Deterministic payment terms identifier
    pub terms_id: String,
    /// Amount in USDC microlamports (6 decimals)
    pub amount_usdc: u64,
    /// Payment period in seconds
    pub period_secs: u64,
    /// Unix timestamp when payment terms were created
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

/// Event emitted when a payment execution succeeds but remaining allowance is low
///
/// This warning event alerts off-chain systems and users when the delegate allowance
/// drops below a recommended threshold (2x the payment price). While the current payment
/// succeeded, the low allowance may cause the next payment to fail if not topped up.
///
/// This addresses the allowance management UX concern from audit finding L-3, where
/// users may successfully start a payment agreement with multi-period allowance but find
/// payments failing if allowance drops below the single-period price.
///
/// Off-chain systems should monitor this event to:
/// - Send notifications to users to increase their allowance
/// - Display warnings in UI before the next payment date
/// - Trigger automated allowance top-up workflows
/// - Generate analytics on allowance management patterns
#[event]
pub struct LowAllowanceWarning {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms with low allowance
    pub payment_terms: Pubkey,
    /// The payer who needs to increase allowance
    pub payer: Pubkey,
    /// Current remaining allowance (in USDC micro-units)
    pub current_allowance: u64,
    /// Recommended minimum allowance (2x payment price)
    pub recommended_allowance: u64,
    /// Payment price for reference (in USDC micro-units)
    pub payment_price: u64,
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

/// Event emitted when a delegate mismatch is detected during payment execution
///
/// This warning event alerts off-chain systems and users when the token account's
/// current delegate does not match the expected global protocol delegate PDA.
///
/// **Global Delegate Architecture**: This protocol uses a single global delegate PDA
/// shared by all payees and payment agreements. The global delegate enables users to
/// have payment agreements with multiple payees using the same token account without delegate conflicts.
///
/// **Scenarios that trigger this event:**
/// 1. User manually revoked the global delegate → All payment agreements on this account stop executing
/// 2. User approved a different program's delegate → Token account now delegated elsewhere
/// 3. Delegate was never approved → Payment agreement was created without proper delegate setup
/// 4. User is using the token account for other programs → Delegate overwritten by another protocol
///
/// Off-chain systems should monitor this event to:
/// - Alert users that their payment agreement is non-functional due to delegate mismatch
/// - Recommend re-approving the global delegate to restore all payment agreements
/// - Guide users to reactivate affected payment agreements
/// - Display clear information about the global delegate requirement
/// - Track delegate revocation patterns for user support
///
/// **Recovery**: The user needs to re-approve the global protocol delegate on their token
/// account, then reactivate any affected payment agreements. Once the global delegate is approved,
/// ALL payee payment agreements will be able to execute again.
///
/// **Important**: This payment agreement will NOT execute until the delegate is corrected and the
/// payment agreement is reactivated.
#[event]
pub struct DelegateMismatchWarning {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms with delegate mismatch
    pub payment_terms: Pubkey,
    /// The payer whose token account has incorrect delegate
    pub payer: Pubkey,
    /// The expected global protocol delegate PDA
    pub expected_delegate: Pubkey,
    /// The actual delegate currently set on the token account (may be None or from another program)
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

/// Event emitted when a payee's volume tier is upgraded
///
/// Volume tiers upgrade automatically based on 30-day rolling payment volume.
/// This event provides transparency and auditability for tier changes.
///
/// Tier upgrades immediately affect the platform fee rate applied to future payments:
/// - Standard → Growth: $10K monthly volume reached (2.5% → 2.0% fee)
/// - Growth → Scale: $100K monthly volume reached (2.0% → 1.5% fee)
///
/// Off-chain systems can monitor this event to:
/// - Track payee growth and volume progression
/// - Generate analytics on tier adoption and revenue impact
/// - Alert payees of automatic tier upgrades
/// - Maintain audit trails for fee calculations and billing
#[event]
pub struct VolumeTierUpgraded {
    /// The payee account whose tier upgraded
    pub payee: Pubkey,
    /// The previous tier before the upgrade
    pub old_tier: crate::state::VolumeTier,
    /// The new tier after the upgrade
    pub new_tier: crate::state::VolumeTier,
    /// Current 30-day rolling volume that triggered the upgrade
    pub monthly_volume_usdc: u64,
    /// The new platform fee in basis points corresponding to the new tier
    pub new_platform_fee_bps: u16,
}

/// Event emitted when payment terms' pricing or period are updated
///
/// This event provides transparency for all payment term modifications made by payee authority.
/// Term updates affect existing payment agreements starting from their next payment.
/// Off-chain systems can monitor this event to:
/// - Track pricing changes and revenue impacts
/// - Alert payers of upcoming term changes
/// - Generate analytics on payment terms evolution patterns
/// - Maintain audit trails for payment agreement management
#[event]
pub struct PaymentTermsUpdated {
    /// The payment terms account whose terms were updated
    pub payment_terms: Pubkey,
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The old amount before update (if amount was updated)
    pub old_amount: Option<u64>,
    /// The new amount after update (if amount was updated)
    pub new_amount: Option<u64>,
    /// The old period before update (if period was updated)
    pub old_period: Option<u64>,
    /// The new period after update (if period was updated)
    pub new_period: Option<u64>,
    /// Payee authority who performed the update
    pub updated_by: Pubkey,
}

