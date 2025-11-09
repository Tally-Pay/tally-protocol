//! Event parsing utilities for Tally program events and structured receipts

use crate::{error::Result, TallyError};
use anchor_client::solana_sdk::{signature::Signature, transaction::TransactionError};
use anchor_lang::prelude::*;
use base64::prelude::*;
use chrono;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event emitted when a subscription is successfully started
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Event emitted when a subscription is successfully renewed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Renewed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being renewed
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for the renewal (in USDC micro-units)
    pub amount: u64,
    /// The keeper who executed the renewal
    pub keeper: Pubkey,
    /// The fee paid to the keeper (in USDC micro-units)
    pub keeper_fee: u64,
}

/// Event emitted when a subscription is canceled
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Canceled {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being canceled
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
}

/// Event emitted when a subscription payment fails
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Event emitted when a previously canceled subscription is reactivated
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Event emitted when a subscription account is closed and rent is reclaimed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct SubscriptionClosed {
    /// The subscription plan that was closed
    pub plan: Pubkey,
    /// The subscriber's public key who closed the subscription and received the rent
    pub subscriber: Pubkey,
}

/// Event emitted when a plan's active status is changed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ProgramPaused {
    /// Platform authority who initiated the pause
    pub authority: Pubkey,
    /// Unix timestamp when program was paused
    pub timestamp: i64,
}

/// Event emitted when the program is unpaused
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ProgramUnpaused {
    /// Platform authority who initiated the unpause
    pub authority: Pubkey,
    /// Unix timestamp when program was unpaused
    pub timestamp: i64,
}

/// Event emitted when a subscription renewal succeeds but remaining allowance is low
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Volume tier for platform fee calculation based on monthly payment volume
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeTier {
    /// Standard tier: Up to $10K monthly volume (0.25% platform fee)
    Standard,
    /// Growth tier: $10K - $100K monthly volume (0.20% platform fee)
    Growth,
    /// Scale tier: Over $100K monthly volume (0.15% platform fee)
    Scale,
}

// Manual Borsh implementation to avoid ambiguity
impl anchor_lang::AnchorSerialize for VolumeTier {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Self::Standard => writer.write_all(&[0]),
            Self::Growth => writer.write_all(&[1]),
            Self::Scale => writer.write_all(&[2]),
        }
    }
}

impl anchor_lang::AnchorDeserialize for VolumeTier {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        if buf.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Empty buffer",
            ));
        }
        let discriminant = buf[0];
        *buf = &buf[1..];
        match discriminant {
            0 => Ok(Self::Standard),
            1 => Ok(Self::Growth),
            2 => Ok(Self::Scale),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid VolumeTier discriminant",
            )),
        }
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut discriminant = [0u8; 1];
        reader.read_exact(&mut discriminant)?;
        match discriminant[0] {
            0 => Ok(Self::Standard),
            1 => Ok(Self::Growth),
            2 => Ok(Self::Scale),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid VolumeTier discriminant",
            )),
        }
    }
}

/// Event emitted when a payee's volume tier is upgraded based on payment volume
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeTierUpgraded {
    /// The merchant account whose tier was upgraded
    pub merchant: Pubkey,
    /// The previous tier before the upgrade
    pub old_tier: VolumeTier,
    /// The new tier after the upgrade
    pub new_tier: VolumeTier,
    /// The rolling 30-day volume that triggered the upgrade
    pub monthly_volume_usdc: u64,
    /// The new platform fee in basis points corresponding to the new tier
    pub new_platform_fee_bps: u16,
}

/// Event emitted when a plan's pricing or terms are updated
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct TrialConverted {
    /// The subscription account that converted from trial to paid
    pub subscription: Pubkey,
    /// The subscriber who converted to paid
    pub subscriber: Pubkey,
    /// The subscription plan that was converted
    pub plan: Pubkey,
}

/// All possible Tally program events
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TallyEvent {
    /// Subscription started
    Subscribed(Subscribed),
    /// Subscription reactivated
    SubscriptionReactivated(SubscriptionReactivated),
    /// Subscription renewed
    Renewed(Renewed),
    /// Subscription canceled
    Canceled(Canceled),
    /// Subscription closed
    SubscriptionClosed(SubscriptionClosed),
    /// Payment failed
    PaymentFailed(PaymentFailed),
    /// Plan status changed
    PlanStatusChanged(PlanStatusChanged),
    /// Config initialized
    ConfigInitialized(ConfigInitialized),
    /// Merchant initialized
    MerchantInitialized(MerchantInitialized),
    /// Plan created
    PlanCreated(PlanCreated),
    /// Program paused
    ProgramPaused(ProgramPaused),
    /// Program unpaused
    ProgramUnpaused(ProgramUnpaused),
    /// Low allowance warning
    LowAllowanceWarning(LowAllowanceWarning),
    /// Fees withdrawn
    FeesWithdrawn(FeesWithdrawn),
    /// Delegate mismatch warning
    DelegateMismatchWarning(DelegateMismatchWarning),
    /// Config updated
    ConfigUpdated(ConfigUpdated),
    /// Volume tier upgraded based on payment volume
    VolumeTierUpgraded(VolumeTierUpgraded),
    /// Plan terms updated
    PlanTermsUpdated(PlanTermsUpdated),
    /// Trial started
    TrialStarted(TrialStarted),
    /// Trial converted
    TrialConverted(TrialConverted),
}

/// Enhanced parsed event with transaction context for RPC queries and WebSocket streaming
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedEventWithContext {
    /// Transaction signature that contains this event
    pub signature: Signature,
    /// Slot number where transaction was processed
    pub slot: u64,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction success status
    pub success: bool,
    /// The parsed Tally event
    pub event: TallyEvent,
    /// Log index within the transaction
    pub log_index: usize,
}

/// WebSocket-friendly event data for dashboard streaming
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamableEventData {
    /// Event type as string
    pub event_type: String,
    /// Merchant PDA
    pub merchant_pda: String,
    /// Transaction signature
    pub transaction_signature: String,
    /// Event timestamp
    pub timestamp: i64,
    /// Event metadata
    pub metadata: HashMap<String, String>,
    /// Amount involved (if applicable)
    pub amount: Option<u64>,
    /// Plan address (if applicable)
    pub plan_address: Option<String>,
    /// Subscription address (if applicable)
    pub subscription_address: Option<String>,
}

impl ParsedEventWithContext {
    /// Create a new `ParsedEventWithContext` from components
    #[must_use]
    pub const fn new(
        signature: Signature,
        slot: u64,
        block_time: Option<i64>,
        success: bool,
        event: TallyEvent,
        log_index: usize,
    ) -> Self {
        Self {
            signature,
            slot,
            block_time,
            success,
            event,
            log_index,
        }
    }

    /// Convert to streamable event data for WebSocket
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn to_streamable(&self) -> StreamableEventData {
        let mut metadata = HashMap::new();

        let (event_type, merchant_pda, plan_address, amount) = match &self.event {
            TallyEvent::Subscribed(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                ("subscribed".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), Some(e.amount))
            }
            TallyEvent::SubscriptionReactivated(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("total_renewals".to_string(), e.total_renewals.to_string());
                metadata.insert("original_created_ts".to_string(), e.original_created_ts.to_string());
                ("subscription_reactivated".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), Some(e.amount))
            }
            TallyEvent::Renewed(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("keeper".to_string(), e.keeper.to_string());
                metadata.insert("keeper_fee".to_string(), e.keeper_fee.to_string());
                ("renewed".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), Some(e.amount))
            }
            TallyEvent::Canceled(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                ("canceled".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::SubscriptionClosed(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                ("subscription_closed".to_string(), String::new(), Some(e.plan.to_string()), None)
            }
            TallyEvent::PaymentFailed(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("reason".to_string(), e.reason.clone());
                ("payment_failed".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::PlanStatusChanged(e) => {
                metadata.insert("active".to_string(), e.active.to_string());
                metadata.insert("changed_by".to_string(), e.changed_by.clone());
                ("plan_status_changed".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::ConfigInitialized(e) => {
                metadata.insert("platform_authority".to_string(), e.platform_authority.to_string());
                metadata.insert("allowed_mint".to_string(), e.allowed_mint.to_string());
                ("config_initialized".to_string(), String::new(), None, None)
            }
            TallyEvent::MerchantInitialized(e) => {
                metadata.insert("authority".to_string(), e.authority.to_string());
                metadata.insert("usdc_mint".to_string(), e.usdc_mint.to_string());
                metadata.insert("treasury_ata".to_string(), e.treasury_ata.to_string());
                ("merchant_initialized".to_string(), e.merchant.to_string(), None, None)
            }
            TallyEvent::PlanCreated(e) => {
                metadata.insert("plan_id".to_string(), e.plan_id.clone());
                metadata.insert("price_usdc".to_string(), e.price_usdc.to_string());
                metadata.insert("period_secs".to_string(), e.period_secs.to_string());
                metadata.insert("grace_secs".to_string(), e.grace_secs.to_string());
                metadata.insert("name".to_string(), e.name.clone());
                ("plan_created".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::ProgramPaused(e) => {
                metadata.insert("authority".to_string(), e.authority.to_string());
                ("program_paused".to_string(), String::new(), None, None)
            }
            TallyEvent::ProgramUnpaused(e) => {
                metadata.insert("authority".to_string(), e.authority.to_string());
                ("program_unpaused".to_string(), String::new(), None, None)
            }
            TallyEvent::LowAllowanceWarning(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("current_allowance".to_string(), e.current_allowance.to_string());
                metadata.insert("recommended_allowance".to_string(), e.recommended_allowance.to_string());
                metadata.insert("plan_price".to_string(), e.plan_price.to_string());
                ("low_allowance_warning".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::FeesWithdrawn(e) => {
                metadata.insert("platform_authority".to_string(), e.platform_authority.to_string());
                metadata.insert("destination".to_string(), e.destination.to_string());
                ("fees_withdrawn".to_string(), String::new(), None, Some(e.amount))
            }
            TallyEvent::DelegateMismatchWarning(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("expected_delegate".to_string(), e.expected_delegate.to_string());
                if let Some(actual) = &e.actual_delegate {
                    metadata.insert("actual_delegate".to_string(), actual.to_string());
                }
                ("delegate_mismatch_warning".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::ConfigUpdated(e) => {
                metadata.insert("updated_by".to_string(), e.updated_by.to_string());
                metadata.insert("keeper_fee_bps".to_string(), e.keeper_fee_bps.to_string());
                ("config_updated".to_string(), String::new(), None, None)
            }
            TallyEvent::VolumeTierUpgraded(e) => {
                metadata.insert("old_tier".to_string(), format!("{:?}", e.old_tier));
                metadata.insert("new_tier".to_string(), format!("{:?}", e.new_tier));
                metadata.insert("monthly_volume_usdc".to_string(), e.monthly_volume_usdc.to_string());
                metadata.insert("new_platform_fee_bps".to_string(), e.new_platform_fee_bps.to_string());
                ("volume_tier_upgraded".to_string(), e.merchant.to_string(), None, None)
            }
            TallyEvent::PlanTermsUpdated(e) => {
                metadata.insert("updated_by".to_string(), e.updated_by.to_string());
                if let Some(old_price) = e.old_price {
                    metadata.insert("old_price".to_string(), old_price.to_string());
                }
                if let Some(new_price) = e.new_price {
                    metadata.insert("new_price".to_string(), new_price.to_string());
                }
                ("plan_terms_updated".to_string(), e.merchant.to_string(), Some(e.plan.to_string()), None)
            }
            TallyEvent::TrialStarted(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("subscription".to_string(), e.subscription.to_string());
                metadata.insert("trial_ends_at".to_string(), e.trial_ends_at.to_string());
                ("trial_started".to_string(), String::new(), Some(e.plan.to_string()), None)
            }
            TallyEvent::TrialConverted(e) => {
                metadata.insert("subscriber".to_string(), e.subscriber.to_string());
                metadata.insert("subscription".to_string(), e.subscription.to_string());
                ("trial_converted".to_string(), String::new(), Some(e.plan.to_string()), None)
            }
        };
        metadata.insert("slot".to_string(), self.slot.to_string());
        metadata.insert("success".to_string(), self.success.to_string());

        // Generate subscription address for events that have plan + subscriber
        let subscription_address = if plan_address.is_some() && metadata.contains_key("subscriber")
        {
            let subscriber_str = metadata.get("subscriber").map_or("unknown", String::as_str);
            Some(format!(
                "subscription_{}_{}",
                plan_address.as_deref().unwrap_or("unknown"),
                subscriber_str
            ))
        } else {
            None
        };

        StreamableEventData {
            event_type,
            merchant_pda,
            transaction_signature: self.signature.to_string(),
            timestamp: self.block_time.unwrap_or(0),
            metadata,
            amount,
            plan_address,
            subscription_address,
        }
    }

    /// Check if this event was successful
    #[must_use]
    pub const fn is_successful(&self) -> bool {
        self.success
    }

    /// Get the merchant pubkey from the event
    #[must_use]
    pub const fn get_merchant(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::Subscribed(e) => Some(e.merchant),
            TallyEvent::SubscriptionReactivated(e) => Some(e.merchant),
            TallyEvent::Renewed(e) => Some(e.merchant),
            TallyEvent::Canceled(e) => Some(e.merchant),
            TallyEvent::PaymentFailed(e) => Some(e.merchant),
            TallyEvent::PlanStatusChanged(e) => Some(e.merchant),
            TallyEvent::MerchantInitialized(e) => Some(e.merchant),
            TallyEvent::PlanCreated(e) => Some(e.merchant),
            TallyEvent::LowAllowanceWarning(e) => Some(e.merchant),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.merchant),
            TallyEvent::VolumeTierUpgraded(e) => Some(e.merchant),
            TallyEvent::PlanTermsUpdated(e) => Some(e.merchant),
            _ => None,
        }
    }

    /// Get the plan pubkey from the event
    #[must_use]
    pub const fn get_plan(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::Subscribed(e) => Some(e.plan),
            TallyEvent::SubscriptionReactivated(e) => Some(e.plan),
            TallyEvent::Renewed(e) => Some(e.plan),
            TallyEvent::Canceled(e) => Some(e.plan),
            TallyEvent::SubscriptionClosed(e) => Some(e.plan),
            TallyEvent::PaymentFailed(e) => Some(e.plan),
            TallyEvent::PlanStatusChanged(e) => Some(e.plan),
            TallyEvent::PlanCreated(e) => Some(e.plan),
            TallyEvent::LowAllowanceWarning(e) => Some(e.plan),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.plan),
            TallyEvent::PlanTermsUpdated(e) => Some(e.plan),
            TallyEvent::TrialStarted(e) => Some(e.plan),
            TallyEvent::TrialConverted(e) => Some(e.plan),
            _ => None,
        }
    }

    /// Get the subscriber pubkey from the event
    #[must_use]
    pub const fn get_subscriber(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::Subscribed(e) => Some(e.subscriber),
            TallyEvent::SubscriptionReactivated(e) => Some(e.subscriber),
            TallyEvent::Renewed(e) => Some(e.subscriber),
            TallyEvent::Canceled(e) => Some(e.subscriber),
            TallyEvent::SubscriptionClosed(e) => Some(e.subscriber),
            TallyEvent::PaymentFailed(e) => Some(e.subscriber),
            TallyEvent::LowAllowanceWarning(e) => Some(e.subscriber),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.subscriber),
            TallyEvent::TrialStarted(e) => Some(e.subscriber),
            TallyEvent::TrialConverted(e) => Some(e.subscriber),
            _ => None,
        }
    }

    /// Get the amount from the event (if applicable)
    #[must_use]
    pub const fn get_amount(&self) -> Option<u64> {
        match &self.event {
            TallyEvent::Subscribed(e) => Some(e.amount),
            TallyEvent::SubscriptionReactivated(e) => Some(e.amount),
            TallyEvent::Renewed(e) => Some(e.amount),
            TallyEvent::FeesWithdrawn(e) => Some(e.amount),
            _ => None,
        }
    }

    /// Get event type as string for display
    #[must_use]
    pub fn get_event_type_string(&self) -> String {
        match &self.event {
            TallyEvent::Subscribed(_) => "Subscribed".to_string(),
            TallyEvent::SubscriptionReactivated(_) => "SubscriptionReactivated".to_string(),
            TallyEvent::Renewed(_) => "Renewed".to_string(),
            TallyEvent::Canceled(_) => "Canceled".to_string(),
            TallyEvent::SubscriptionClosed(_) => "SubscriptionClosed".to_string(),
            TallyEvent::PaymentFailed(_) => "PaymentFailed".to_string(),
            TallyEvent::PlanStatusChanged(_) => "PlanStatusChanged".to_string(),
            TallyEvent::ConfigInitialized(_) => "ConfigInitialized".to_string(),
            TallyEvent::MerchantInitialized(_) => "MerchantInitialized".to_string(),
            TallyEvent::PlanCreated(_) => "PlanCreated".to_string(),
            TallyEvent::ProgramPaused(_) => "ProgramPaused".to_string(),
            TallyEvent::ProgramUnpaused(_) => "ProgramUnpaused".to_string(),
            TallyEvent::LowAllowanceWarning(_) => "LowAllowanceWarning".to_string(),
            TallyEvent::FeesWithdrawn(_) => "FeesWithdrawn".to_string(),
            TallyEvent::DelegateMismatchWarning(_) => "DelegateMismatchWarning".to_string(),
            TallyEvent::ConfigUpdated(_) => "ConfigUpdated".to_string(),
            TallyEvent::VolumeTierUpgraded(_) => "VolumeTierUpgraded".to_string(),
            TallyEvent::PlanTermsUpdated(_) => "PlanTermsUpdated".to_string(),
            TallyEvent::TrialStarted(_) => "TrialStarted".to_string(),
            TallyEvent::TrialConverted(_) => "TrialConverted".to_string(),
        }
    }

    /// Format amount as USDC (6 decimal places)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn format_amount(&self) -> Option<f64> {
        self.get_amount().map(|amount| amount as f64 / 1_000_000.0)
    }

    /// Get timestamp as formatted string
    #[must_use]
    pub fn format_timestamp(&self) -> String {
        self.block_time.map_or_else(
            || "Pending".to_string(),
            |timestamp| {
                chrono::DateTime::from_timestamp(timestamp, 0)
                    .map_or_else(|| "Unknown".to_string(), |dt| dt.to_rfc3339())
            },
        )
    }

    /// Check if this event affects revenue
    #[must_use]
    pub const fn affects_revenue(&self) -> bool {
        matches!(
            &self.event,
            TallyEvent::Subscribed(_) | TallyEvent::Renewed(_)
        )
    }

    /// Check if this event affects subscription count
    #[must_use]
    pub const fn affects_subscription_count(&self) -> bool {
        matches!(
            &self.event,
            TallyEvent::Subscribed(_) | TallyEvent::Canceled(_)
        )
    }

    /// Get the payment failure reason (if applicable)
    #[must_use]
    pub fn get_failure_reason(&self) -> Option<&str> {
        match &self.event {
            TallyEvent::PaymentFailed(e) => Some(&e.reason),
            _ => None,
        }
    }
}

/// Compute the 8-byte discriminator for an Anchor event
/// Formula: first 8 bytes of SHA256("event:<EventName>")
fn compute_event_discriminator(event_name: &str) -> [u8; 8] {
    use anchor_lang::solana_program::hash;
    let preimage = format!("event:{event_name}");
    let hash_result = hash::hash(preimage.as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash_result.to_bytes()[..8]);
    discriminator
}

/// Get all event discriminators for fast lookup
fn get_event_discriminators() -> HashMap<[u8; 8], &'static str> {
    let mut discriminators = HashMap::new();
    discriminators.insert(compute_event_discriminator("Subscribed"), "Subscribed");
    discriminators.insert(compute_event_discriminator("Renewed"), "Renewed");
    discriminators.insert(compute_event_discriminator("Canceled"), "Canceled");
    discriminators.insert(
        compute_event_discriminator("PaymentFailed"),
        "PaymentFailed",
    );
    discriminators
}

/// Structured receipt for a Tally transaction
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TallyReceipt {
    /// Transaction signature
    pub signature: Signature,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction slot
    pub slot: u64,
    /// Whether the transaction was successful
    pub success: bool,
    /// Transaction error if any
    pub error: Option<String>,
    /// Parsed Tally events from this transaction
    pub events: Vec<TallyEvent>,
    /// Program logs from the transaction
    pub logs: Vec<String>,
    /// Compute units consumed
    pub compute_units_consumed: Option<u64>,
    /// Transaction fee in lamports
    pub fee: u64,
}

/// Parse Tally events from transaction logs with transaction context
///
/// # Arguments
/// * `logs` - The transaction logs to parse
/// * `program_id` - The Tally program ID to filter events
/// * `signature` - Transaction signature
/// * `slot` - Transaction slot
/// * `block_time` - Block time
/// * `success` - Transaction success status
///
/// # Returns
/// * `Ok(Vec<ParsedEventWithContext>)` - Parsed events with context
/// * `Err(TallyError)` - If parsing fails
pub fn parse_events_with_context(
    logs: &[String],
    program_id: &Pubkey,
    signature: Signature,
    slot: u64,
    block_time: Option<i64>,
    success: bool,
) -> Result<Vec<ParsedEventWithContext>> {
    let events = parse_events_from_logs(logs, program_id)?;
    let mut parsed_events = Vec::new();

    for (log_index, event) in events.into_iter().enumerate() {
        parsed_events.push(ParsedEventWithContext::new(
            signature, slot, block_time, success, event, log_index,
        ));
    }

    Ok(parsed_events)
}

/// Parse Tally events from transaction logs
///
/// # Arguments
/// * `logs` - The transaction logs to parse
/// * `program_id` - The Tally program ID to filter events
///
/// # Returns
/// * `Ok(Vec<TallyEvent>)` - Parsed events
/// * `Err(TallyError)` - If parsing fails
pub fn parse_events_from_logs(logs: &[String], program_id: &Pubkey) -> Result<Vec<TallyEvent>> {
    let mut events = Vec::new();
    let program_data_prefix = format!("Program data: {program_id} ");

    for log in logs {
        if let Some(data_start) = log.find(&program_data_prefix) {
            let event_data = &log[data_start.saturating_add(program_data_prefix.len())..];
            if let Ok(event) = parse_single_event(event_data) {
                events.push(event);
            }
        }
    }

    Ok(events)
}

/// Parse a single event from base64-encoded data
///
/// Anchor events are encoded as: discriminator (8 bytes) + borsh-serialized event data
/// The discriminator is computed as the first 8 bytes of SHA256("event:<EventName>")
pub fn parse_single_event(data: &str) -> Result<TallyEvent> {
    // Decode base64 data
    let decoded_data = base64::prelude::BASE64_STANDARD
        .decode(data)
        .map_err(|e| TallyError::ParseError(format!("Failed to decode base64: {e}")))?;

    // Check minimum length for discriminator (8 bytes)
    if decoded_data.len() < 8 {
        return Err(TallyError::ParseError(
            "Event data too short, must be at least 8 bytes for discriminator".to_string(),
        ));
    }

    // Extract discriminator (first 8 bytes)
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&decoded_data[..8]);

    // Get event data (remaining bytes after discriminator)
    let event_data = &decoded_data[8..];

    // Determine event type based on discriminator
    let discriminators = get_event_discriminators();
    let event_type = discriminators.get(&discriminator).ok_or_else(|| {
        TallyError::ParseError(format!("Unknown event discriminator: {discriminator:?}"))
    })?;

    // Deserialize the event data using Borsh
    match *event_type {
        "Subscribed" => {
            let event = Subscribed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Subscribed event: {e}"))
            })?;
            Ok(TallyEvent::Subscribed(event))
        }
        "Renewed" => {
            let event = Renewed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Renewed event: {e}"))
            })?;
            Ok(TallyEvent::Renewed(event))
        }
        "Canceled" => {
            let event = Canceled::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Canceled event: {e}"))
            })?;
            Ok(TallyEvent::Canceled(event))
        }
        "PaymentFailed" => {
            let event = PaymentFailed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize PaymentFailed event: {e}"))
            })?;
            Ok(TallyEvent::PaymentFailed(event))
        }
        _ => Err(TallyError::ParseError(format!(
            "Unhandled event type: {event_type}"
        ))),
    }
}

/// Parameters for creating a structured receipt
pub struct ReceiptParams {
    /// Transaction signature
    pub signature: Signature,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction slot
    pub slot: u64,
    /// Whether transaction was successful
    pub success: bool,
    /// Transaction error if any
    pub error: Option<TransactionError>,
    /// Transaction logs
    pub logs: Vec<String>,
    /// Compute units consumed
    pub compute_units_consumed: Option<u64>,
    /// Transaction fee
    pub fee: u64,
    /// Program ID to parse events for
    pub program_id: Pubkey,
}

/// Create a structured receipt from transaction components
///
/// # Arguments
/// * `params` - Receipt creation parameters
///
/// # Returns
/// * `Ok(TallyReceipt)` - Structured receipt
/// * `Err(TallyError)` - If parsing fails
pub fn create_receipt(params: ReceiptParams) -> Result<TallyReceipt> {
    let events = parse_events_from_logs(&params.logs, &params.program_id)?;

    Ok(TallyReceipt {
        signature: params.signature,
        block_time: params.block_time,
        slot: params.slot,
        success: params.success,
        error: params.error.map(|e| format!("{e:?}")),
        events,
        logs: params.logs,
        compute_units_consumed: params.compute_units_consumed,
        fee: params.fee,
    })
}

/// Create a structured receipt from transaction components (legacy compatibility)
///
/// # Arguments
/// * `signature` - Transaction signature
/// * `block_time` - Block time (Unix timestamp)
/// * `slot` - Transaction slot
/// * `success` - Whether transaction was successful
/// * `error` - Transaction error if any
/// * `logs` - Transaction logs
/// * `compute_units_consumed` - Compute units consumed
/// * `fee` - Transaction fee
/// * `program_id` - Program ID to parse events for
///
/// # Returns
/// * `Ok(TallyReceipt)` - Structured receipt
/// * `Err(TallyError)` - If parsing fails
#[allow(clippy::too_many_arguments)] // Legacy function, will be deprecated
pub fn create_receipt_legacy(
    signature: Signature,
    block_time: Option<i64>,
    slot: u64,
    success: bool,
    error: Option<TransactionError>,
    logs: Vec<String>,
    compute_units_consumed: Option<u64>,
    fee: u64,
    program_id: &Pubkey,
) -> Result<TallyReceipt> {
    create_receipt(ReceiptParams {
        signature,
        block_time,
        slot,
        success,
        error,
        logs,
        compute_units_consumed,
        fee,
        program_id: *program_id,
    })
}

/// Extract memo from transaction logs
///
/// # Arguments
/// * `logs` - Transaction logs to search
///
/// # Returns
/// * `Option<String>` - Found memo, if any
#[must_use]
pub fn extract_memo_from_logs(logs: &[String]) -> Option<String> {
    for log in logs {
        if log.starts_with("Program log: Memo (len ") {
            // Format: "Program log: Memo (len N): \"message\""
            if let Some(start) = log.find("): \"") {
                let memo_start = start.saturating_add(4); // Skip "): \""
                if let Some(end) = log.rfind('"') {
                    if end > memo_start {
                        return Some(log[memo_start..end].to_string());
                    }
                }
            }
        } else if log.starts_with("Program log: ") && log.contains("memo:") {
            // Alternative memo format
            if let Some(memo_start) = log.find("memo:") {
                let memo_content = &log[memo_start.saturating_add(5)..].trim();
                return Some((*memo_content).to_string());
            }
        }
    }
    None
}

/// Find the first Tally event of a specific type in a receipt
impl TallyReceipt {
    /// Get the first Subscribed event, if any
    #[must_use]
    pub fn get_subscribed_event(&self) -> Option<&Subscribed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Subscribed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Renewed event, if any
    #[must_use]
    pub fn get_renewed_event(&self) -> Option<&Renewed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Renewed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Canceled event, if any
    #[must_use]
    pub fn get_canceled_event(&self) -> Option<&Canceled> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Canceled(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first `PaymentFailed` event, if any
    #[must_use]
    pub fn get_payment_failed_event(&self) -> Option<&PaymentFailed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentFailed(e) => Some(e),
            _ => None,
        })
    }

    /// Extract memo from transaction logs
    #[must_use]
    pub fn extract_memo(&self) -> Option<String> {
        extract_memo_from_logs(&self.logs)
    }

    /// Check if this receipt represents a successful subscription operation
    #[must_use]
    pub fn is_subscription_success(&self) -> bool {
        self.success
            && (self.get_subscribed_event().is_some()
                || self.get_renewed_event().is_some()
                || self.get_canceled_event().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_extract_memo_from_logs() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: Memo (len 12): \"Test message\"".to_string(),
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units"
                .to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Test message".to_string()));
    }

    #[test]
    fn test_extract_memo_alternative_format() {
        let logs = vec!["Program log: Processing memo: Hello world".to_string()];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_memo_none() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units"
                .to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, None);
    }

    #[test]
    fn test_tally_receipt_event_getters() {
        let signature = Signature::default();
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let subscribed_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000, // 1 USDC
        };

        let receipt = TallyReceipt {
            signature,
            block_time: Some(1_640_995_200), // 2022-01-01
            slot: 100,
            success: true,
            error: None,
            events: vec![TallyEvent::Subscribed(subscribed_event.clone())],
            logs: vec![],
            compute_units_consumed: Some(5000),
            fee: 5000,
        };

        assert_eq!(receipt.get_subscribed_event(), Some(&subscribed_event));
        assert_eq!(receipt.get_renewed_event(), None);
        assert_eq!(receipt.get_canceled_event(), None);
        assert_eq!(receipt.get_payment_failed_event(), None);
        assert!(receipt.is_subscription_success());
    }

    #[test]
    fn test_tally_receipt_failed_transaction() {
        let signature = Signature::default();

        let receipt = TallyReceipt {
            signature,
            block_time: Some(1_640_995_200),
            slot: 100,
            success: false,
            error: Some("InsufficientFunds".to_string()),
            events: vec![],
            logs: vec![],
            compute_units_consumed: Some(1000),
            fee: 5000,
        };

        assert!(!receipt.is_subscription_success());
    }

    #[test]
    fn test_create_receipt() {
        let signature = Signature::default();
        let program_id = crate::program_id();

        let receipt = create_receipt(ReceiptParams {
            signature,
            block_time: Some(1_640_995_200),
            slot: 100,
            success: true,
            error: None,
            logs: vec!["Program invoked".to_string()],
            compute_units_consumed: Some(5000),
            fee: 5000,
            program_id,
        })
        .unwrap();

        assert_eq!(receipt.signature, signature);
        assert_eq!(receipt.slot, 100);
        assert!(receipt.success);
        assert_eq!(receipt.error, None);
        assert_eq!(receipt.fee, 5000);
    }

    // Helper function to create base64-encoded event data for testing
    fn create_test_event_data(event_name: &str, event_struct: &impl AnchorSerialize) -> String {
        let discriminator = compute_event_discriminator(event_name);
        let mut event_data = Vec::new();
        event_data.extend_from_slice(&discriminator);
        event_struct.serialize(&mut event_data).unwrap();
        base64::prelude::BASE64_STANDARD.encode(event_data)
    }

    #[test]
    fn test_compute_event_discriminator() {
        let subscribed_disc = compute_event_discriminator("Subscribed");
        let renewed_disc = compute_event_discriminator("Renewed");
        let canceled_disc = compute_event_discriminator("Canceled");
        let payment_failed_disc = compute_event_discriminator("PaymentFailed");

        // All discriminators should be unique
        assert_ne!(subscribed_disc, renewed_disc);
        assert_ne!(subscribed_disc, canceled_disc);
        assert_ne!(subscribed_disc, payment_failed_disc);
        assert_ne!(renewed_disc, canceled_disc);
        assert_ne!(renewed_disc, payment_failed_disc);
        assert_ne!(canceled_disc, payment_failed_disc);

        // Discriminators should be deterministic
        assert_eq!(subscribed_disc, compute_event_discriminator("Subscribed"));
        assert_eq!(renewed_disc, compute_event_discriminator("Renewed"));
    }

    #[test]
    fn test_get_event_discriminators() {
        let discriminators = get_event_discriminators();

        assert_eq!(discriminators.len(), 4);
        assert!(discriminators.contains_key(&compute_event_discriminator("Subscribed")));
        assert!(discriminators.contains_key(&compute_event_discriminator("Renewed")));
        assert!(discriminators.contains_key(&compute_event_discriminator("Canceled")));
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentFailed")));
    }

    #[test]
    fn test_parse_subscribed_event() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 5_000_000, // 5 USDC
        };

        let encoded_data = create_test_event_data("Subscribed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 5_000_000);
            }
            _ => panic!("Expected Subscribed event"),
        }
    }

    #[test]
    fn test_parse_renewed_event() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let event = Renewed {
            merchant,
            plan,
            subscriber,
            amount: 10_000_000, // 10 USDC
            keeper,
            keeper_fee: 50_000, // 0.05 USDC keeper fee
        };

        let encoded_data = create_test_event_data("Renewed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Renewed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 10_000_000);
            }
            _ => panic!("Expected Renewed event"),
        }
    }

    #[test]
    fn test_parse_canceled_event() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = Canceled {
            merchant,
            plan,
            subscriber,
        };

        let encoded_data = create_test_event_data("Canceled", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Canceled(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
            }
            _ => panic!("Expected Canceled event"),
        }
    }

    #[test]
    fn test_parse_payment_failed_event() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = PaymentFailed {
            merchant,
            plan,
            subscriber,
            reason: "Insufficient funds".to_string(),
        };

        let encoded_data = create_test_event_data("PaymentFailed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentFailed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.reason, "Insufficient funds");
            }
            _ => panic!("Expected PaymentFailed event"),
        }
    }

    #[test]
    fn test_parse_single_event_invalid_base64() {
        let result = parse_single_event("invalid_base64_!@#$%");
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to decode base64"));
        }
    }

    #[test]
    fn test_parse_single_event_too_short() {
        // Create data with only 6 bytes (less than 8-byte discriminator requirement)
        let short_data = base64::prelude::BASE64_STANDARD.encode(vec![1, 2, 3, 4, 5, 6]);
        let result = parse_single_event(&short_data);

        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Event data too short"));
        }
    }

    #[test]
    fn test_parse_single_event_unknown_discriminator() {
        // Create data with unknown discriminator
        let mut data = vec![0xFF; 8]; // Unknown discriminator
        data.extend_from_slice(&[1, 2, 3, 4]); // Some event data
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Unknown event discriminator"));
        }
    }

    #[test]
    fn test_parse_single_event_malformed_event_data() {
        // Create data with correct discriminator but malformed event data
        let discriminator = compute_event_discriminator("Subscribed");
        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // Malformed data that can't be deserialized as Subscribed
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to deserialize Subscribed event"));
        }
    }

    #[test]
    fn test_parse_events_from_logs() {
        let program_id = crate::program_id();
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let subscribed_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let canceled_event = Canceled {
            merchant,
            plan,
            subscriber,
        };

        let subscribed_data = create_test_event_data("Subscribed", &subscribed_event);
        let canceled_data = create_test_event_data("Canceled", &canceled_event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", program_id, subscribed_data),
            "Program log: Some other log".to_string(),
            format!("Program data: {} {}", program_id, canceled_data),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        assert_eq!(events.len(), 2);

        // Check first event (Subscribed)
        match &events[0] {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected first event to be Subscribed"),
        }

        // Check second event (Canceled)
        match &events[1] {
            TallyEvent::Canceled(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
            }
            _ => panic!("Expected second event to be Canceled"),
        }
    }

    #[test]
    fn test_parse_events_from_logs_with_malformed_data() {
        let program_id = crate::program_id();
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let valid_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let valid_data = create_test_event_data("Subscribed", &valid_event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", program_id, valid_data),
            format!("Program data: {} invalid_base64_!@#$%", program_id), // Malformed data - should be skipped
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        // Only the valid event should be parsed, malformed one should be skipped
        assert_eq!(events.len(), 1);

        match &events[0] {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected event to be Subscribed"),
        }
    }

    #[test]
    fn test_parse_events_from_logs_empty() {
        let program_id = crate::program_id();
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: No events here".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_parse_events_from_logs_different_program() {
        let program_id = crate::program_id();
        let other_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let event_data = create_test_event_data("Subscribed", &event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", other_program_id, event_data), // Different program
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        // Should not parse events from different program
        assert_eq!(events.len(), 0);
    }
}
