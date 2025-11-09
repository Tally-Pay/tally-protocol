//! Program account types and structures

use anchor_lang::prelude::*;
use serde::{Deserialize, Serialize};

/// Volume tier determines platform fee rate based on monthly payment volume
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VolumeTier {
    /// Standard tier: Up to $10K monthly volume, 0.25% platform fee (25 basis points)
    Standard = 0,
    /// Growth tier: $10K - $100K monthly volume, 0.20% platform fee (20 basis points)
    Growth = 1,
    /// Scale tier: Over $100K monthly volume, 0.15% platform fee (15 basis points)
    Scale = 2,
}

// Manual borsh implementations for VolumeTier to avoid version conflicts
impl anchor_lang::AnchorSerialize for VolumeTier {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let discriminant = *self as u8;
        anchor_lang::AnchorSerialize::serialize(&discriminant, writer)
    }
}

impl anchor_lang::AnchorDeserialize for VolumeTier {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let discriminant: u8 = anchor_lang::AnchorDeserialize::deserialize(buf)?;
        Self::from_discriminant(discriminant)
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid VolumeTier discriminant: {discriminant}")
            ))
    }

    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let discriminant: u8 = anchor_lang::AnchorDeserialize::deserialize_reader(reader)?;
        Self::from_discriminant(discriminant)
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid VolumeTier discriminant: {discriminant}")
            ))
    }
}

impl VolumeTier {
    /// Returns the platform fee in basis points for this tier
    #[must_use]
    pub const fn platform_fee_bps(self) -> u16 {
        match self {
            Self::Standard => 25, // 0.25%
            Self::Growth => 20,   // 0.20%
            Self::Scale => 15,    // 0.15%
        }
    }

    /// Create from u8 discriminant
    #[must_use]
    pub const fn from_discriminant(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Standard),
            1 => Some(Self::Growth),
            2 => Some(Self::Scale),
            _ => None,
        }
    }

    /// Determines tier based on 30-day rolling volume
    #[must_use]
    pub const fn from_monthly_volume(volume_usdc: u64) -> Self {
        const GROWTH_THRESHOLD: u64 = 10_000_000_000; // $10K
        const SCALE_THRESHOLD: u64 = 100_000_000_000; // $100K

        if volume_usdc >= SCALE_THRESHOLD {
            Self::Scale
        } else if volume_usdc >= GROWTH_THRESHOLD {
            Self::Growth
        } else {
            Self::Standard
        }
    }
}

/// Payee account stores payment recipient configuration and settings
/// PDA seeds: ["payee", authority]
///
/// # Volume Tracking
///
/// The Payee account tracks rolling 30-day payment volume to automatically
/// determine the payee's fee tier. Volume resets after 30 days of inactivity.
///
/// # Account Size: 122 bytes
/// - Discriminator: 8 bytes
/// - authority: 32 bytes
/// - `usdc_mint`: 32 bytes
/// - `treasury_ata`: 32 bytes
/// - `volume_tier`: 1 byte
/// - `monthly_volume_usdc`: 8 bytes
/// - `last_volume_update_ts`: 8 bytes
/// - bump: 1 byte
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Payee {
    /// Payee authority (signer for payee operations)
    pub authority: Pubkey,
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey,
    /// Payee's USDC treasury ATA (where payment revenues are sent)
    pub treasury_ata: Pubkey,
    /// Current volume tier (automatically calculated from `monthly_volume_usdc`)
    ///
    /// This tier determines the platform fee rate:
    /// - Standard: 0.25% (up to $10K monthly)
    /// - Growth: 0.20% ($10K-$100K monthly)
    /// - Scale: 0.15% (>$100K monthly)
    ///
    /// The tier is recalculated on every payment execution.
    pub volume_tier: VolumeTier,
    /// Rolling 30-day payment volume in USDC microlamports (6 decimals)
    ///
    /// This field accumulates total payment volume processed by this payee
    /// over a rolling 30-day window. It resets to zero if no payments are
    /// processed for 30 days.
    pub monthly_volume_usdc: u64,
    /// Unix timestamp of last volume calculation
    ///
    /// Used to determine if 30-day window has elapsed and volume should reset.
    pub last_volume_update_ts: i64,
    /// PDA bump seed
    pub bump: u8,
}

/// `PaymentTerms` account defines payment schedule and amount for recurring payments
/// PDA seeds: [`"payment_terms"`, `payee`, `terms_id`]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentTerms {
    /// Reference to the payee PDA
    pub payee: Pubkey,
    /// Deterministic payment terms identifier (string as bytes, padded to 32)
    pub terms_id: [u8; 32],
    /// Payment amount in USDC microlamports (6 decimals)
    pub amount_usdc: u64,
    /// Payment period in seconds (payment frequency)
    pub period_secs: u64,
}

/// `PaymentAgreement` account tracks recurring payment relationship between payer and payee
/// PDA seeds: [`"payment_agreement"`, `payment_terms`, `payer`]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PaymentAgreement {
    /// Reference to the payment terms PDA
    pub payment_terms: Pubkey,
    /// Payer's pubkey (the one making payments)
    pub payer: Pubkey,
    /// Unix timestamp for next payment execution
    pub next_payment_ts: i64,
    /// Whether payment agreement is active
    pub active: bool,
    /// Number of payments executed under this agreement
    pub payment_count: u32,
    /// Unix timestamp when agreement was created
    pub created_ts: i64,
    /// Last payment amount for audit purposes
    pub last_amount: u64,
    /// Unix timestamp when last payment was executed (prevents double-payment attacks)
    pub last_payment_ts: i64,
    /// PDA bump seed
    pub bump: u8,
}

/// Arguments for initializing a payee
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct InitPayeeArgs {
    /// USDC mint address
    pub usdc_mint: Pubkey,
    /// Treasury ATA for receiving payee revenues
    pub treasury_ata: Pubkey,
}

/// Arguments for creating payment terms
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct CreatePaymentTermsArgs {
    /// Unique terms identifier (original string)
    pub terms_id: String,
    /// Padded `terms_id` bytes for PDA seeds (must match program constraint calculation)
    pub terms_id_bytes: [u8; 32],
    /// Payment amount in USDC microlamports (6 decimals)
    pub amount_usdc: u64,
    /// Payment period in seconds
    pub period_secs: u64,
}

/// Arguments for starting a payment agreement
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize, Default,
)]
pub struct StartAgreementArgs {
    /// Allowance periods multiplier (default from config if 0)
    pub allowance_periods: u8,
}

/// Arguments for executing a payment
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize, Default,
)]
pub struct ExecutePaymentArgs {
    // No args needed - payment execution driven by executor
}

/// Arguments for pausing a payment agreement
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PauseAgreementArgs {
    // No args needed for pausing
}

/// Arguments for admin fee withdrawal
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct AdminWithdrawFeesArgs {
    /// Amount to withdraw in USDC microlamports
    pub amount: u64,
}

/// Global configuration account for program constants and settings
/// PDA seeds: `["config"]`
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Config {
    /// Platform authority pubkey for admin operations
    pub platform_authority: Pubkey,
    /// Pending authority for two-step authority transfer
    pub pending_authority: Option<Pubkey>,
    /// Maximum platform fee in basis points (e.g., 1000 = 10%)
    pub max_platform_fee_bps: u16,
    /// Minimum platform fee in basis points (e.g., 50 = 0.5%)
    pub min_platform_fee_bps: u16,
    /// Minimum payment agreement period in seconds (e.g., 86400 = 24 hours)
    pub min_period_seconds: u64,
    /// Default allowance periods multiplier (e.g., 3)
    pub default_allowance_periods: u8,
    /// Allowed token mint address (e.g., official USDC mint)
    /// This prevents payees from using fake or arbitrary tokens
    pub allowed_mint: Pubkey,
    /// Maximum withdrawal amount per transaction in USDC microlamports
    /// Prevents accidental or malicious drainage of entire treasury
    pub max_withdrawal_amount: u64,
    /// Maximum grace period in seconds (e.g., 604800 = 7 days)
    /// Prevents excessively long grace periods that increase payee payment risk
    pub max_grace_period_seconds: u64,
    /// Emergency pause state - when true, all user-facing operations are disabled
    /// This allows the platform authority to halt operations in case of security incidents
    pub paused: bool,
    /// Keeper fee in basis points (e.g., 25 = 0.25%)
    /// This fee is paid to the transaction caller (keeper) to incentivize decentralized renewal network
    /// Capped at 100 basis points (1%) to prevent excessive keeper fees
    pub keeper_fee_bps: u16,
    /// PDA bump seed
    pub bump: u8,
}

/// Arguments for initializing global program configuration
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct InitConfigArgs {
    /// Platform authority pubkey for admin operations
    pub platform_authority: Pubkey,
    /// Maximum platform fee in basis points (e.g., 1000 = 10%)
    pub max_platform_fee_bps: u16,
    /// Minimum platform fee in basis points (e.g., 50 = 0.5%)
    pub min_platform_fee_bps: u16,
    /// Minimum payment agreement period in seconds (e.g., 86400 = 24 hours)
    pub min_period_seconds: u64,
    /// Default allowance periods multiplier (e.g., 3)
    pub default_allowance_periods: u8,
    /// Allowed USDC mint address
    pub allowed_mint: Pubkey,
    /// Maximum withdrawal amount per transaction
    pub max_withdrawal_amount: u64,
    /// Maximum grace period in seconds
    pub max_grace_period_seconds: u64,
    /// Keeper fee in basis points
    pub keeper_fee_bps: u16,
}

impl PaymentTerms {
    /// Convert `terms_id` bytes to string, trimming null bytes
    #[must_use]
    pub fn terms_id_str(&self) -> String {
        String::from_utf8_lossy(&self.terms_id)
            .trim_end_matches('\0')
            .to_string()
    }

    /// Get payment amount in USDC (human readable, with 6 decimals)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn amount_usdc_formatted(&self) -> f64 {
        self.amount_usdc as f64 / 1_000_000.0
    }

    /// Get period in human readable format
    #[must_use]
    pub fn period_formatted(&self) -> String {
        let days = self.period_secs / 86400;
        if days == 1 {
            "1 day".to_string()
        } else if days == 7 {
            "1 week".to_string()
        } else if days == 30 {
            "1 month".to_string()
        } else if days == 365 {
            "1 year".to_string()
        } else {
            format!("{days} days")
        }
    }
}

impl CreatePaymentTermsArgs {
    /// Convert `terms_id` string to padded 32-byte array
    #[must_use]
    pub fn terms_id_bytes_from_string(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let id_bytes = self.terms_id.as_bytes();
        let len = id_bytes.len().min(32);
        bytes[..len].copy_from_slice(&id_bytes[..len]);
        bytes
    }
}

/// Arguments for closing a payment agreement
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct CloseAgreementArgs {
    // No args needed for closing
}

/// Arguments for initiating authority transfer
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct TransferAuthorityArgs {
    /// The new authority to transfer to
    pub new_authority: Pubkey,
}

/// Arguments for accepting authority transfer
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct AcceptAuthorityArgs {
    // No arguments needed - signer validation is sufficient
}

/// Arguments for canceling authority transfer
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct CancelAuthorityTransferArgs {
    // No arguments needed - signer validation is sufficient
}

/// Arguments for pausing the program
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PauseArgs {}

/// Arguments for unpausing the program
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct UnpauseArgs {}

/// Arguments for updating global program configuration
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(
    Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct UpdateConfigArgs {
    /// Keeper fee in basis points
    pub keeper_fee_bps: Option<u16>,
    /// Maximum withdrawal amount
    pub max_withdrawal_amount: Option<u64>,
    /// Maximum grace period in seconds
    pub max_grace_period_seconds: Option<u64>,
    /// Minimum platform fee in basis points
    pub min_platform_fee_bps: Option<u16>,
    /// Maximum platform fee in basis points
    pub max_platform_fee_bps: Option<u16>,
    /// Minimum period in seconds
    pub min_period_seconds: Option<u64>,
    /// Default allowance periods
    pub default_allowance_periods: Option<u8>,
}



