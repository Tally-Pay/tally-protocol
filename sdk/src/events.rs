//! Event parsing utilities for Tally program events and structured receipts

use crate::{error::Result, TallyError};
use anchor_client::solana_sdk::{signature::Signature, transaction::TransactionError};
use anchor_lang::prelude::*;
use base64::prelude::*;
use chrono;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event emitted when a payment agreement is successfully started
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentAgreementStarted {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being agreed to
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid for the first payment (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a payment is successfully executed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentExecuted {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being executed
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid (in USDC micro-units)
    pub amount: u64,
    /// The keeper who executed the payment
    pub keeper: Pubkey,
    /// The fee paid to the keeper (in USDC micro-units)
    pub keeper_fee: u64,
}

/// Event emitted when a payment agreement is paused
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentAgreementPaused {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being paused
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
}

/// Event emitted when a payment fails
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Event emitted when a previously paused payment agreement is resumed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentAgreementResumed {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms being resumed
    pub payment_terms: Pubkey,
    /// The payer's public key
    pub payer: Pubkey,
    /// The amount paid for resumption (in USDC micro-units)
    pub amount: u64,
    /// Cumulative number of payments across all agreement sessions
    pub total_payments: u32,
    /// Original agreement creation timestamp (preserved from first session)
    pub original_created_ts: i64,
}

/// Event emitted when a payment agreement account is closed and rent is reclaimed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentAgreementClosed {
    /// The payment terms that was closed
    pub payment_terms: Pubkey,
    /// The payer's public key who closed the agreement and received the rent
    pub payer: Pubkey,
}

/// Event emitted when payment terms' active status is changed
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
    /// Minimum payment agreement period in seconds
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PayeeInitialized {
    /// The payee PDA account
    pub payee: Pubkey,
    /// Payee authority (signer for payee operations)
    pub authority: Pubkey,
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey,
    /// Payee's USDC treasury ATA
    pub treasury_ata: Pubkey,
    /// Platform fee in basis points
    pub platform_fee_bps: u16,
    /// Unix timestamp when payee was initialized
    pub timestamp: i64,
}

/// Event emitted when payment terms are created
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentTermsCreated {
    /// The payment terms PDA account
    pub payment_terms: Pubkey,
    /// Reference to the payee PDA
    pub payee: Pubkey,
    /// Deterministic payment terms identifier
    pub terms_id: String,
    /// Price in USDC microlamports (6 decimals)
    pub amount_usdc: u64,
    /// Payment period in seconds
    pub period_secs: u64,
    /// Grace period for payments in seconds
    pub grace_secs: u64,
    /// Payment terms display name
    pub name: String,
    /// Unix timestamp when payment terms were created
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

/// Event emitted when a payment succeeds but remaining allowance is low
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct LowAllowanceWarning {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms with low allowance
    pub payment_terms: Pubkey,
    /// The payer who needs to increase allowance
    pub payer: Pubkey,
    /// Current remaining allowance (in USDC micro-units)
    pub current_allowance: u64,
    /// Recommended minimum allowance (2x payment amount)
    pub recommended_allowance: u64,
    /// Payment amount for reference (in USDC micro-units)
    pub payment_amount: u64,
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

/// Event emitted when a delegate mismatch is detected during payment execution
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct DelegateMismatchWarning {
    /// The payee who owns the payment terms
    pub payee: Pubkey,
    /// The payment terms with delegate mismatch
    pub payment_terms: Pubkey,
    /// The payer whose token account has incorrect delegate
    pub payer: Pubkey,
    /// The expected delegate PDA for this payee
    pub expected_delegate: Pubkey,
    /// The actual delegate currently set on the token account (may be None or different payee)
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
    /// The payee account whose tier was upgraded
    pub payee: Pubkey,
    /// The previous tier before the upgrade
    pub old_tier: VolumeTier,
    /// The new tier after the upgrade
    pub new_tier: VolumeTier,
    /// The rolling 30-day volume that triggered the upgrade
    pub monthly_volume_usdc: u64,
    /// The new platform fee in basis points corresponding to the new tier
    pub new_platform_fee_bps: u16,
}

/// Event emitted when payment terms pricing or terms are updated
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentTermsUpdated {
    /// The payment terms account that was updated
    pub payment_terms: Pubkey,
    /// The payee who owns the payment terms
    pub payee: Pubkey,
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
    /// Payee authority who performed the update
    pub updated_by: Pubkey,
}

/// All possible Tally program events
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TallyEvent {
    /// Payment agreement started
    PaymentAgreementStarted(PaymentAgreementStarted),
    /// Payment agreement resumed
    PaymentAgreementResumed(PaymentAgreementResumed),
    /// Payment executed
    PaymentExecuted(PaymentExecuted),
    /// Payment agreement paused
    PaymentAgreementPaused(PaymentAgreementPaused),
    /// Payment agreement closed
    PaymentAgreementClosed(PaymentAgreementClosed),
    /// Payment failed
    PaymentFailed(PaymentFailed),
    /// Payment terms status changed
    PaymentTermsStatusChanged(PaymentTermsStatusChanged),
    /// Config initialized
    ConfigInitialized(ConfigInitialized),
    /// Payee initialized
    PayeeInitialized(PayeeInitialized),
    /// Payment terms created
    PaymentTermsCreated(PaymentTermsCreated),
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
    /// Payment terms updated
    PaymentTermsUpdated(PaymentTermsUpdated),
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
    /// Payee PDA
    pub payee_pda: String,
    /// Transaction signature
    pub transaction_signature: String,
    /// Event timestamp
    pub timestamp: i64,
    /// Event metadata
    pub metadata: HashMap<String, String>,
    /// Amount involved (if applicable)
    pub amount: Option<u64>,
    /// `PaymentTerms` address (if applicable)
    pub payment_terms_address: Option<String>,
    /// `PaymentAgreement` address (if applicable)
    pub agreement_address: Option<String>,
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

        let (event_type, payee_pda, payment_terms_address, amount) = match &self.event {
            TallyEvent::PaymentAgreementStarted(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                ("payment_agreement_started".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), Some(e.amount))
            }
            TallyEvent::PaymentAgreementResumed(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                metadata.insert("total_payments".to_string(), e.total_payments.to_string());
                metadata.insert("original_created_ts".to_string(), e.original_created_ts.to_string());
                ("payment_agreement_resumed".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), Some(e.amount))
            }
            TallyEvent::PaymentExecuted(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                metadata.insert("keeper".to_string(), e.keeper.to_string());
                metadata.insert("keeper_fee".to_string(), e.keeper_fee.to_string());
                ("payment_executed".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), Some(e.amount))
            }
            TallyEvent::PaymentAgreementPaused(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                ("payment_agreement_paused".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
            }
            TallyEvent::PaymentAgreementClosed(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                ("payment_agreement_closed".to_string(), String::new(), Some(e.payment_terms.to_string()), None)
            }
            TallyEvent::PaymentFailed(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                metadata.insert("reason".to_string(), e.reason.clone());
                ("payment_failed".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
            }
            TallyEvent::PaymentTermsStatusChanged(e) => {
                metadata.insert("active".to_string(), e.active.to_string());
                metadata.insert("changed_by".to_string(), e.changed_by.clone());
                ("payment_terms_status_changed".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
            }
            TallyEvent::ConfigInitialized(e) => {
                metadata.insert("platform_authority".to_string(), e.platform_authority.to_string());
                metadata.insert("allowed_mint".to_string(), e.allowed_mint.to_string());
                ("config_initialized".to_string(), String::new(), None, None)
            }
            TallyEvent::PayeeInitialized(e) => {
                metadata.insert("authority".to_string(), e.authority.to_string());
                metadata.insert("usdc_mint".to_string(), e.usdc_mint.to_string());
                metadata.insert("treasury_ata".to_string(), e.treasury_ata.to_string());
                ("payee_initialized".to_string(), e.payee.to_string(), None, None)
            }
            TallyEvent::PaymentTermsCreated(e) => {
                metadata.insert("terms_id".to_string(), e.terms_id.clone());
                metadata.insert("amount_usdc".to_string(), e.amount_usdc.to_string());
                metadata.insert("period_secs".to_string(), e.period_secs.to_string());
                metadata.insert("grace_secs".to_string(), e.grace_secs.to_string());
                metadata.insert("name".to_string(), e.name.clone());
                ("payment_terms_created".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
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
                metadata.insert("payer".to_string(), e.payer.to_string());
                metadata.insert("current_allowance".to_string(), e.current_allowance.to_string());
                metadata.insert("recommended_allowance".to_string(), e.recommended_allowance.to_string());
                metadata.insert("payment_amount".to_string(), e.payment_amount.to_string());
                ("low_allowance_warning".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
            }
            TallyEvent::FeesWithdrawn(e) => {
                metadata.insert("platform_authority".to_string(), e.platform_authority.to_string());
                metadata.insert("destination".to_string(), e.destination.to_string());
                ("fees_withdrawn".to_string(), String::new(), None, Some(e.amount))
            }
            TallyEvent::DelegateMismatchWarning(e) => {
                metadata.insert("payer".to_string(), e.payer.to_string());
                metadata.insert("expected_delegate".to_string(), e.expected_delegate.to_string());
                if let Some(actual) = &e.actual_delegate {
                    metadata.insert("actual_delegate".to_string(), actual.to_string());
                }
                ("delegate_mismatch_warning".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
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
                ("volume_tier_upgraded".to_string(), e.payee.to_string(), None, None)
            }
            TallyEvent::PaymentTermsUpdated(e) => {
                metadata.insert("updated_by".to_string(), e.updated_by.to_string());
                if let Some(old_price) = e.old_price {
                    metadata.insert("old_price".to_string(), old_price.to_string());
                }
                if let Some(new_price) = e.new_price {
                    metadata.insert("new_price".to_string(), new_price.to_string());
                }
                ("payment_terms_updated".to_string(), e.payee.to_string(), Some(e.payment_terms.to_string()), None)
            }
        };
        metadata.insert("slot".to_string(), self.slot.to_string());
        metadata.insert("success".to_string(), self.success.to_string());

        // Generate payment agreement address for events that have payment_terms + payer
        let agreement_address = if payment_terms_address.is_some() && metadata.contains_key("payer")
        {
            let payer_str = metadata.get("payer").map_or("unknown", String::as_str);
            Some(format!(
                "payment_agreement_{}_{}",
                payment_terms_address.as_deref().unwrap_or("unknown"),
                payer_str
            ))
        } else {
            None
        };

        StreamableEventData {
            event_type,
            payee_pda,
            transaction_signature: self.signature.to_string(),
            timestamp: self.block_time.unwrap_or(0),
            metadata,
            amount,
            payment_terms_address,
            agreement_address,
        }
    }

    /// Check if this event was successful
    #[must_use]
    pub const fn is_successful(&self) -> bool {
        self.success
    }

    /// Get the payee pubkey from the event
    #[must_use]
    pub const fn get_payee(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::PaymentAgreementStarted(e) => Some(e.payee),
            TallyEvent::PaymentAgreementResumed(e) => Some(e.payee),
            TallyEvent::PaymentExecuted(e) => Some(e.payee),
            TallyEvent::PaymentAgreementPaused(e) => Some(e.payee),
            TallyEvent::PaymentFailed(e) => Some(e.payee),
            TallyEvent::PaymentTermsStatusChanged(e) => Some(e.payee),
            TallyEvent::PayeeInitialized(e) => Some(e.payee),
            TallyEvent::PaymentTermsCreated(e) => Some(e.payee),
            TallyEvent::LowAllowanceWarning(e) => Some(e.payee),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.payee),
            TallyEvent::VolumeTierUpgraded(e) => Some(e.payee),
            TallyEvent::PaymentTermsUpdated(e) => Some(e.payee),
            _ => None,
        }
    }

    /// Get the payment terms pubkey from the event
    #[must_use]
    pub const fn get_payment_terms(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::PaymentAgreementStarted(e) => Some(e.payment_terms),
            TallyEvent::PaymentAgreementResumed(e) => Some(e.payment_terms),
            TallyEvent::PaymentExecuted(e) => Some(e.payment_terms),
            TallyEvent::PaymentAgreementPaused(e) => Some(e.payment_terms),
            TallyEvent::PaymentAgreementClosed(e) => Some(e.payment_terms),
            TallyEvent::PaymentFailed(e) => Some(e.payment_terms),
            TallyEvent::PaymentTermsStatusChanged(e) => Some(e.payment_terms),
            TallyEvent::PaymentTermsCreated(e) => Some(e.payment_terms),
            TallyEvent::LowAllowanceWarning(e) => Some(e.payment_terms),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.payment_terms),
            TallyEvent::PaymentTermsUpdated(e) => Some(e.payment_terms),
            _ => None,
        }
    }

    /// Get the payer pubkey from the event
    #[must_use]
    pub const fn get_payer(&self) -> Option<Pubkey> {
        match &self.event {
            TallyEvent::PaymentAgreementStarted(e) => Some(e.payer),
            TallyEvent::PaymentAgreementResumed(e) => Some(e.payer),
            TallyEvent::PaymentExecuted(e) => Some(e.payer),
            TallyEvent::PaymentAgreementPaused(e) => Some(e.payer),
            TallyEvent::PaymentAgreementClosed(e) => Some(e.payer),
            TallyEvent::PaymentFailed(e) => Some(e.payer),
            TallyEvent::LowAllowanceWarning(e) => Some(e.payer),
            TallyEvent::DelegateMismatchWarning(e) => Some(e.payer),
            _ => None,
        }
    }

    /// Get the amount from the event (if applicable)
    #[must_use]
    pub const fn get_amount(&self) -> Option<u64> {
        match &self.event {
            TallyEvent::PaymentAgreementStarted(e) => Some(e.amount),
            TallyEvent::PaymentAgreementResumed(e) => Some(e.amount),
            TallyEvent::PaymentExecuted(e) => Some(e.amount),
            TallyEvent::FeesWithdrawn(e) => Some(e.amount),
            _ => None,
        }
    }

    /// Get event type as string for display
    #[must_use]
    pub fn get_event_type_string(&self) -> String {
        match &self.event {
            TallyEvent::PaymentAgreementStarted(_) => "PaymentAgreementStarted".to_string(),
            TallyEvent::PaymentAgreementResumed(_) => "PaymentAgreementResumed".to_string(),
            TallyEvent::PaymentExecuted(_) => "PaymentExecuted".to_string(),
            TallyEvent::PaymentAgreementPaused(_) => "PaymentAgreementPaused".to_string(),
            TallyEvent::PaymentAgreementClosed(_) => "PaymentAgreementClosed".to_string(),
            TallyEvent::PaymentFailed(_) => "PaymentFailed".to_string(),
            TallyEvent::PaymentTermsStatusChanged(_) => "PaymentTermsStatusChanged".to_string(),
            TallyEvent::ConfigInitialized(_) => "ConfigInitialized".to_string(),
            TallyEvent::PayeeInitialized(_) => "PayeeInitialized".to_string(),
            TallyEvent::PaymentTermsCreated(_) => "PaymentTermsCreated".to_string(),
            TallyEvent::ProgramPaused(_) => "ProgramPaused".to_string(),
            TallyEvent::ProgramUnpaused(_) => "ProgramUnpaused".to_string(),
            TallyEvent::LowAllowanceWarning(_) => "LowAllowanceWarning".to_string(),
            TallyEvent::FeesWithdrawn(_) => "FeesWithdrawn".to_string(),
            TallyEvent::DelegateMismatchWarning(_) => "DelegateMismatchWarning".to_string(),
            TallyEvent::ConfigUpdated(_) => "ConfigUpdated".to_string(),
            TallyEvent::VolumeTierUpgraded(_) => "VolumeTierUpgraded".to_string(),
            TallyEvent::PaymentTermsUpdated(_) => "PaymentTermsUpdated".to_string(),
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
            TallyEvent::PaymentAgreementStarted(_) | TallyEvent::PaymentExecuted(_)
        )
    }

    /// Check if this event affects payment agreement count
    #[must_use]
    pub const fn affects_agreement_count(&self) -> bool {
        matches!(
            &self.event,
            TallyEvent::PaymentAgreementStarted(_) | TallyEvent::PaymentAgreementPaused(_)
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
    discriminators.insert(compute_event_discriminator("PaymentAgreementStarted"), "PaymentAgreementStarted");
    discriminators.insert(compute_event_discriminator("PaymentExecuted"), "PaymentExecuted");
    discriminators.insert(compute_event_discriminator("PaymentAgreementPaused"), "PaymentAgreementPaused");
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
        "PaymentAgreementStarted" => {
            let event = PaymentAgreementStarted::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize PaymentAgreementStarted event: {e}"))
            })?;
            Ok(TallyEvent::PaymentAgreementStarted(event))
        }
        "PaymentExecuted" => {
            let event = PaymentExecuted::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize PaymentExecuted event: {e}"))
            })?;
            Ok(TallyEvent::PaymentExecuted(event))
        }
        "PaymentAgreementPaused" => {
            let event = PaymentAgreementPaused::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize PaymentAgreementPaused event: {e}"))
            })?;
            Ok(TallyEvent::PaymentAgreementPaused(event))
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
    /// Get the first `PaymentAgreementStarted` event, if any
    #[must_use]
    pub fn get_agreement_started_event(&self) -> Option<&PaymentAgreementStarted> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentAgreementStarted(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first `PaymentExecuted` event, if any
    #[must_use]
    pub fn get_payment_executed_event(&self) -> Option<&PaymentExecuted> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentExecuted(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first `PaymentAgreementPaused` event, if any
    #[must_use]
    pub fn get_agreement_paused_event(&self) -> Option<&PaymentAgreementPaused> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentAgreementPaused(e) => Some(e),
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

    /// Check if this receipt represents a successful payment agreement operation
    #[must_use]
    pub fn is_agreement_success(&self) -> bool {
        self.success
            && (self.get_agreement_started_event().is_some()
                || self.get_payment_executed_event().is_some()
                || self.get_agreement_paused_event().is_some())
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
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let agreement_started_event = PaymentAgreementStarted {
            payee,
            payment_terms,
            payer,
            amount: 1_000_000, // 1 USDC
        };

        let receipt = TallyReceipt {
            signature,
            block_time: Some(1_640_995_200), // 2022-01-01
            slot: 100,
            success: true,
            error: None,
            events: vec![TallyEvent::PaymentAgreementStarted(agreement_started_event.clone())],
            logs: vec![],
            compute_units_consumed: Some(5000),
            fee: 5000,
        };

        assert_eq!(receipt.get_agreement_started_event(), Some(&agreement_started_event));
        assert_eq!(receipt.get_payment_executed_event(), None);
        assert_eq!(receipt.get_agreement_paused_event(), None);
        assert_eq!(receipt.get_payment_failed_event(), None);
        assert!(receipt.is_agreement_success());
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

        assert!(!receipt.is_agreement_success());
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
        let agreement_started_disc = compute_event_discriminator("PaymentAgreementStarted");
        let payment_executed_disc = compute_event_discriminator("PaymentExecuted");
        let agreement_paused_disc = compute_event_discriminator("PaymentAgreementPaused");
        let payment_failed_disc = compute_event_discriminator("PaymentFailed");

        // All discriminators should be unique
        assert_ne!(agreement_started_disc, payment_executed_disc);
        assert_ne!(agreement_started_disc, agreement_paused_disc);
        assert_ne!(agreement_started_disc, payment_failed_disc);
        assert_ne!(payment_executed_disc, agreement_paused_disc);
        assert_ne!(payment_executed_disc, payment_failed_disc);
        assert_ne!(agreement_paused_disc, payment_failed_disc);

        // Discriminators should be deterministic
        assert_eq!(agreement_started_disc, compute_event_discriminator("PaymentAgreementStarted"));
        assert_eq!(payment_executed_disc, compute_event_discriminator("PaymentExecuted"));
    }

    #[test]
    fn test_get_event_discriminators() {
        let discriminators = get_event_discriminators();

        assert_eq!(discriminators.len(), 4);
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentAgreementStarted")));
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentExecuted")));
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentAgreementPaused")));
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentFailed")));
    }

    #[test]
    fn test_parse_agreement_started_event() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = PaymentAgreementStarted {
            payee,
            payment_terms,
            payer,
            amount: 5_000_000, // 5 USDC
        };

        let encoded_data = create_test_event_data("PaymentAgreementStarted", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentAgreementStarted(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
                assert_eq!(parsed.amount, 5_000_000);
            }
            _ => panic!("Expected PaymentAgreementStarted event"),
        }
    }

    #[test]
    fn test_parse_payment_executed_event() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let event = PaymentExecuted {
            payee,
            payment_terms,
            payer,
            amount: 10_000_000, // 10 USDC
            keeper,
            keeper_fee: 50_000, // 0.05 USDC keeper fee
        };

        let encoded_data = create_test_event_data("PaymentExecuted", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentExecuted(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
                assert_eq!(parsed.amount, 10_000_000);
            }
            _ => panic!("Expected PaymentExecuted event"),
        }
    }

    #[test]
    fn test_parse_agreement_paused_event() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = PaymentAgreementPaused {
            payee,
            payment_terms,
            payer,
        };

        let encoded_data = create_test_event_data("PaymentAgreementPaused", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentAgreementPaused(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
            }
            _ => panic!("Expected PaymentAgreementPaused event"),
        }
    }

    #[test]
    fn test_parse_payment_failed_event() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = PaymentFailed {
            payee,
            payment_terms,
            payer,
            reason: "Insufficient funds".to_string(),
        };

        let encoded_data = create_test_event_data("PaymentFailed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentFailed(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
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
        let discriminator = compute_event_discriminator("PaymentAgreementStarted");
        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // Malformed data that can't be deserialized as PaymentAgreementStarted
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to deserialize PaymentAgreementStarted event"));
        }
    }

    #[test]
    fn test_parse_events_from_logs() {
        let program_id = crate::program_id();
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let agreement_started_event = PaymentAgreementStarted {
            payee,
            payment_terms,
            payer,
            amount: 1_000_000,
        };

        let agreement_paused_event = PaymentAgreementPaused {
            payee,
            payment_terms,
            payer,
        };

        let started_data = create_test_event_data("PaymentAgreementStarted", &agreement_started_event);
        let paused_data = create_test_event_data("PaymentAgreementPaused", &agreement_paused_event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", program_id, started_data),
            "Program log: Some other log".to_string(),
            format!("Program data: {} {}", program_id, paused_data),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        assert_eq!(events.len(), 2);

        // Check first event (PaymentAgreementStarted)
        match &events[0] {
            TallyEvent::PaymentAgreementStarted(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected first event to be PaymentAgreementStarted"),
        }

        // Check second event (PaymentAgreementPaused)
        match &events[1] {
            TallyEvent::PaymentAgreementPaused(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.payment_terms, payment_terms);
                assert_eq!(parsed.payer, payer);
            }
            _ => panic!("Expected second event to be PaymentAgreementPaused"),
        }
    }

    #[test]
    fn test_parse_events_from_logs_with_malformed_data() {
        let program_id = crate::program_id();
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let valid_event = PaymentAgreementStarted {
            payee,
            payment_terms,
            payer,
            amount: 1_000_000,
        };

        let valid_data = create_test_event_data("PaymentAgreementStarted", &valid_event);

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
            TallyEvent::PaymentAgreementStarted(parsed) => {
                assert_eq!(parsed.payee, payee);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected event to be PaymentAgreementStarted"),
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

        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let event = PaymentAgreementStarted {
            payee,
            payment_terms,
            payer,
            amount: 1_000_000,
        };

        let event_data = create_test_event_data("PaymentAgreementStarted", &event);

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
