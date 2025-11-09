//! Platform administration module for Tally protocol
//!
//! This module provides transaction builders and utilities for platform administrators
//! to manage the global protocol configuration, collect fees, and perform emergency
//! operations.
//!
//! **Note**: These functions require the `admin` feature flag to be enabled.
//!
//! # Admin Operations
//!
//! - **Configuration**: Initialize and update global protocol parameters
//! - **Fee Management**: Withdraw accumulated platform fees
//! - **Authority Transfer**: Securely transfer platform authority
//! - **Emergency Controls**: Pause/unpause protocol operations
//! - **Payee Management**: Update payee tier levels
//!
//! # Security
//!
//! All operations in this module require the platform authority keypair.
//! Authorization is enforced on-chain via the program's `has_one = platform_authority`
//! constraint on the Config account.
//!
//! # Example
//!
//! ```no_run
//! use tally_sdk::admin::*;
//! use anchor_client::solana_sdk::signature::{Keypair, Signer};
//!
//! # fn main() -> tally_sdk::Result<()> {
//! let platform_authority = Keypair::new();
//!
//! // Update global configuration
//! let instruction = update_config()
//!     .platform_authority(platform_authority.pubkey())
//!     .keeper_fee_bps(50)
//!     .build_instruction()?;
//! # Ok(())
//! # }
//! ```

// Re-export admin-related types from program_types
pub use crate::program_types::{
    AdminWithdrawFeesArgs, InitConfigArgs, UpdateConfigArgs,
};

// Re-export admin-related builders from transaction_builder
pub use crate::transaction_builder::{
    accept_authority, admin_withdraw_fees, cancel_authority_transfer, init_config, pause,
    transfer_authority, unpause, update_config, update_payee_tier, AcceptAuthorityBuilder,
    AdminWithdrawFeesBuilder, CancelAuthorityTransferBuilder, InitConfigBuilder, PauseBuilder,
    TransferAuthorityBuilder, UnpauseBuilder, UpdateConfigBuilder, UpdatePayeeTierBuilder,
};
