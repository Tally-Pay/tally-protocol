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
//! use solana_sdk::{pubkey::Pubkey, signature::{Keypair, Signer}};
//! use std::str::FromStr;
//!
//! # fn main() -> tally_sdk::Result<()> {
//! // Initialize client
//! let client = SimpleTallyClient::new("https://api.devnet.solana.com")?;
//!
//! // Compute PDAs
//! let authority = Keypair::new().pubkey();
//! let merchant_pda = pda::merchant_address(&authority)?;
//! let plan_pda = pda::plan_address_from_string(&merchant_pda, "premium_plan")?;
//!
//! // Compute ATAs
//! let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
//! let user_ata = ata::get_associated_token_address_for_mint(&authority, &usdc_mint)?;
//!
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)] // TODO: Add comprehensive error documentation
#![allow(clippy::missing_panics_doc)] // TODO: Add panic documentation where needed

pub mod simple_client;
// pub mod client;  // Disabled for now due to missing discriminator implementations
pub mod ata;
pub mod dashboard;
pub mod dashboard_types;
pub mod error;
pub mod events;
pub mod keypair;
pub mod pda;
pub mod program_types;
pub mod signature;
pub mod transaction_builder;
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
pub use events::{
    TallyEvent, TallyReceipt, Subscribed, Renewed, Canceled, PaymentFailed, ReceiptParams,
    parse_events_from_logs, create_receipt, create_receipt_legacy, extract_memo_from_logs,
};
pub use keypair::load_keypair;
pub use program_types::*;
pub use transaction_builder::{
    admin_withdraw_fees, cancel_subscription, create_merchant, create_plan, init_config,
    start_subscription, AdminWithdrawFeesBuilder, CancelSubscriptionBuilder, CreateMerchantBuilder,
    CreatePlanBuilder, InitConfigBuilder, StartSubscriptionBuilder,
};
pub use validation::*;

// Re-export signature verification utilities
pub use signature::verify_wallet_signature;

// Re-export commonly used external types
pub use solana_sdk;
pub use spl_associated_token_account;
pub use spl_token;

/// Default/fallback program ID (when no environment override is provided)
pub const DEFAULT_PROGRAM_ID: &str = "Fwrs8tRRtw8HwmQZFS3XRRVcKBQhe1nuZ5heB4FgySXV";

/// Get the program ID as a string, checking environment first, then falling back to default
#[must_use]
pub fn program_id_string() -> String {
    std::env::var("PROGRAM_ID").unwrap_or_else(|_| DEFAULT_PROGRAM_ID.to_string())
}

/// Get the program ID as a `Pubkey`
///
/// # Panics
/// Panics if the program ID (from environment or default) is not a valid Pubkey
#[must_use]
pub fn program_id() -> solana_sdk::pubkey::Pubkey {
    program_id_string().parse().expect("Valid program ID")
}
