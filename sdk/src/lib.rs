//! Tally SDK - Rust SDK for the Solana Subscriptions Platform
//!
//! This crate provides a comprehensive Rust SDK for interacting with the Tally
//! subscription program on Solana. It includes utilities for:
//!
//! - Computing Program Derived Addresses (PDAs) and Associated Token Accounts (ATAs)
//! - Building subscription transactions (approve→start, revoke→cancel flows)
//! - Token program detection (SPL Token vs Token-2022)
//!
//! # Example Usage
//!
//! ```no_run
//! use tally_sdk::{pda, ata, SimpleTallyClient};
//! use anchor_client::solana_sdk::pubkey::Pubkey;
//! use anchor_client::solana_sdk::signature::{Keypair, Signer};
//! use std::str::FromStr;
//!
//! # fn main() -> tally_sdk::Result<()> {
//! // Initialize client
//! let client = SimpleTallyClient::new("https://api.devnet.solana.com")?;
//!
//! // Compute PDAs
//! let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
//! let merchant_pda = pda::merchant_address(&authority)?;
//! let plan_pda = pda::plan_address_from_string(&merchant_pda, "premium_plan")?;
//!
//! // Compute ATAs
//! let usdc_mint = Pubkey::try_from("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").map_err(|_| tally_sdk::TallyError::from("Invalid pubkey"))?;
//! let user_ata = ata::get_associated_token_address_for_mint(&authority, &usdc_mint)?;
//!
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod simple_client;
// pub mod client;  // Disabled for now due to missing discriminator implementations
pub mod ata;
pub mod dashboard;
pub mod dashboard_types;
pub mod error;
pub mod event_query;
pub mod events;
pub mod keypair;
pub mod pda;
pub mod program_types;
pub mod signature;
pub mod transaction_builder;
pub mod transaction_utils;
pub mod utils;
pub mod validation;

// Re-export commonly used items
pub use simple_client::SimpleTallyClient;
// pub use client::TallyClient;  // Disabled for now
pub use dashboard::DashboardClient;
pub use dashboard_types::{
    DashboardEvent, DashboardEventType, DashboardSubscription, EventStream, Overview,
    PlanAnalytics, SubscriptionStatus,
};
pub use error::{Result, TallyError};
pub use event_query::{EventQueryClient, EventQueryClientConfig, EventQueryConfig, ParsedEvent};
pub use events::{
    create_receipt, create_receipt_legacy, extract_memo_from_logs, parse_events_from_logs,
    parse_events_with_context, Canceled, ParsedEventWithContext, PaymentFailed, ReceiptParams,
    Renewed, StreamableEventData, Subscribed, TallyEvent, TallyReceipt,
};
pub use keypair::load_keypair;
pub use program_types::*;
pub use transaction_builder::{
    accept_authority, admin_withdraw_fees, cancel_authority_transfer, cancel_subscription,
    close_subscription, create_merchant, create_plan, init_config, pause, renew_subscription,
    start_subscription, transfer_authority, unpause, update_config, update_merchant_tier,
    update_plan_terms, AcceptAuthorityBuilder, AdminWithdrawFeesBuilder,
    CancelAuthorityTransferBuilder, CancelSubscriptionBuilder, CloseSubscriptionBuilder,
    CreateMerchantBuilder, CreatePlanBuilder, InitConfigBuilder, PauseBuilder,
    RenewSubscriptionBuilder, StartSubscriptionBuilder, TransferAuthorityBuilder, UnpauseBuilder,
    UpdateConfigBuilder, UpdateMerchantTierBuilder, UpdatePlanTermsBuilder,
};
pub use validation::*;

// Re-export signature verification and transaction signing utilities
pub use signature::{
    extract_transaction_signature, is_valid_wallet_address, normalize_signature_format,
    prepare_transaction_for_signing, transaction_signing, verify_signed_transaction,
    verify_wallet_signature,
};

// Re-export transaction utilities
pub use transaction_utils::{
    build_transaction, convert_anchor_pubkey, create_memo_instruction, get_user_usdc_ata,
    map_tally_error_to_string, SubscribeTransactionParams,
};

// Re-export general utilities
pub use utils::{
    calculate_next_renewal, format_duration, is_renewal_due, is_subscription_overdue,
    is_valid_pubkey, micro_lamports_to_usdc, system_programs, usdc_to_micro_lamports,
};

// Re-export commonly used external types
pub use anchor_client::solana_account_decoder;
pub use anchor_client::solana_client;
pub use anchor_client::solana_sdk;
pub use anchor_client::ClientError;
pub use anchor_lang::{AnchorDeserialize, AnchorSerialize};
pub use spl_associated_token_account;
pub use spl_token;

use once_cell::sync::Lazy;

/// Program ID loaded from TALLY_PROGRAM_ID environment variable at runtime.
///
/// # Panics
/// Panics if TALLY_PROGRAM_ID environment variable is not set. This is intentional
/// to prevent using the wrong program ID or silently falling back to incorrect defaults.
///
/// # Example
/// ```bash
/// export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111
/// ```
pub static PROGRAM_ID: Lazy<String> = Lazy::new(|| {
    std::env::var("TALLY_PROGRAM_ID")
        .expect("TALLY_PROGRAM_ID environment variable must be set. \
                 Set it to your deployed program ID (localnet/devnet/mainnet).\n\
                 Example: export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111")
});

/// Get the program ID as a string
///
/// # Panics
/// Panics if TALLY_PROGRAM_ID environment variable is not set
///
/// # Example
/// ```bash
/// export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111
/// ```
#[must_use]
pub fn program_id_string() -> String {
    PROGRAM_ID.clone()
}

/// Get the program ID as a `Pubkey`
///
/// # Panics
/// Panics if the program ID (from environment or default) is not a valid Pubkey
#[must_use]
pub fn program_id() -> anchor_client::solana_sdk::pubkey::Pubkey {
    program_id_string().parse().expect("Valid program ID")
}
