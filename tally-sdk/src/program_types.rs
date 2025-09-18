//! Program account types and structures

use anchor_lang::prelude::*;
use serde::{Deserialize, Serialize};

/// Merchant account stores merchant configuration and settings
/// PDA seeds: ["merchant", authority]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Merchant {
    /// Merchant authority (signer for merchant operations)
    pub authority: Pubkey,
    /// Pinned USDC mint address for all transactions
    pub usdc_mint: Pubkey,
    /// Merchant's USDC treasury ATA (where merchant fees are sent)
    pub treasury_ata: Pubkey,
    /// Platform fee in basis points (0-1000, representing 0-10%)
    pub platform_fee_bps: u16,
    /// PDA bump seed
    pub bump: u8,
}

/// Plan account defines subscription plan details
/// PDA seeds: ["plan", merchant, `plan_id`]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Plan {
    /// Reference to the merchant PDA
    pub merchant: Pubkey,
    /// Deterministic plan identifier (string as bytes, padded to 32)
    pub plan_id: [u8; 32],
    /// Price in USDC microlamports (6 decimals)
    pub price_usdc: u64,
    /// Subscription period in seconds
    pub period_secs: u64,
    /// Grace period for renewals in seconds
    pub grace_secs: u64,
    /// Plan display name (string as bytes, padded to 32)
    pub name: [u8; 32],
    /// Whether new subscriptions can be created for this plan
    pub active: bool,
}

/// Subscription account tracks individual user subscriptions
/// PDA seeds: ["subscription", plan, subscriber]
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Subscription {
    /// Reference to the plan PDA
    pub plan: Pubkey,
    /// User's pubkey (the subscriber)
    pub subscriber: Pubkey,
    /// Unix timestamp for next renewal
    pub next_renewal_ts: i64,
    /// Whether subscription is active
    pub active: bool,
    /// Number of renewals processed
    pub renewals: u32,
    /// Unix timestamp when subscription was created
    pub created_ts: i64,
    /// Last charged amount for audit purposes
    pub last_amount: u64,
    /// PDA bump seed
    pub bump: u8,
}

/// Arguments for initializing a merchant
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct InitMerchantArgs {
    /// USDC mint address
    pub usdc_mint: Pubkey,
    /// Treasury ATA for receiving merchant fees
    pub treasury_ata: Pubkey,
    /// Platform fee in basis points (0-1000)
    pub platform_fee_bps: u16,
}

/// Arguments for creating a subscription plan
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct CreatePlanArgs {
    /// Unique plan identifier (will be padded to 32 bytes)
    pub plan_id: String,
    /// Padded `plan_id` bytes for PDA seeds (must match program constraint calculation)
    pub plan_id_bytes: [u8; 32],
    /// Price in USDC microlamports (6 decimals)
    pub price_usdc: u64,
    /// Subscription period in seconds
    pub period_secs: u64,
    /// Grace period for renewals in seconds
    pub grace_secs: u64,
    /// Plan display name (will be padded to 32 bytes)
    pub name: String,
}

/// Arguments for starting a subscription
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct StartSubscriptionArgs {
    /// Allowance periods multiplier (default 3)
    pub allowance_periods: u8,
}

/// Arguments for renewing a subscription
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct RenewSubscriptionArgs {
    /// Expected renewal timestamp (for verification)
    pub expected_renewal_ts: i64,
}

/// Arguments for canceling a subscription
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct CancelSubscriptionArgs;

/// Arguments for admin fee withdrawal
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
    /// Maximum platform fee in basis points (e.g., 1000 = 10%)
    pub max_platform_fee_bps: u16,
    /// Basis points divisor (e.g., 10000 for percentage calculations)
    pub fee_basis_points_divisor: u16,
    /// Minimum subscription period in seconds (e.g., 86400 = 24 hours)
    pub min_period_seconds: u64,
    /// Default allowance periods multiplier (e.g., 3)
    pub default_allowance_periods: u8,
    /// PDA bump seed
    pub bump: u8,
}

/// Arguments for initializing global program configuration
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct InitConfigArgs {
    /// Platform authority pubkey for admin operations
    pub platform_authority: Pubkey,
    /// Maximum platform fee in basis points (e.g., 1000 = 10%)
    pub max_platform_fee_bps: u16,
    /// Basis points divisor (e.g., 10000 for percentage calculations)
    pub fee_basis_points_divisor: u16,
    /// Minimum subscription period in seconds (e.g., 86400 = 24 hours)
    pub min_period_seconds: u64,
    /// Default allowance periods multiplier (e.g., 3)
    pub default_allowance_periods: u8,
}

impl Plan {
    /// Convert `plan_id` bytes to string, trimming null bytes
    #[must_use]
    pub fn plan_id_str(&self) -> String {
        String::from_utf8_lossy(&self.plan_id)
            .trim_end_matches('\0')
            .to_string()
    }

    /// Convert name bytes to string, trimming null bytes
    #[must_use]
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }

    // Compatibility methods for tally-actions migration

    /// Get plan ID as string, removing null padding (alias for `plan_id_str`)
    #[must_use]
    pub fn plan_id_string(&self) -> String {
        self.plan_id_str()
    }

    /// Get plan name as string, removing null padding (alias for `name_str`)
    #[must_use]
    pub fn name_string(&self) -> String {
        self.name_str()
    }

    /// Get plan price in USDC (human readable, with 6 decimals)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn price_usdc_formatted(&self) -> f64 {
        self.price_usdc as f64 / 1_000_000.0
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

impl CreatePlanArgs {
    /// Convert `plan_id` string to padded 32-byte array
    #[must_use]
    pub fn plan_id_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let id_bytes = self.plan_id.as_bytes();
        let len = id_bytes.len().min(32);
        bytes[..len].copy_from_slice(&id_bytes[..len]);
        bytes
    }

    /// Convert name string to padded 32-byte array
    #[must_use]
    pub fn name_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let name_bytes = self.name.as_bytes();
        let len = name_bytes.len().min(32);
        bytes[..len].copy_from_slice(&name_bytes[..len]);
        bytes
    }
}
