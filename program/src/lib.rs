//! Solana Recurring Payments Protocol
//!
//! A Solana-native recurring payment platform implementing delegate-based USDC payments.
//! This program enables payees to create payment terms and collect recurring
//! payments through SPL Token delegate approvals, eliminating the need for user
//! signatures on each payment.
//!
//! ## Core Features
//! - Payee registration with fee configuration
//! - Payment terms creation with flexible pricing and periods
//! - Delegate-based recurring payments using USDC
//! - Automatic payment execution via off-chain executor
//! - Grace period handling for failed payments
//! - Admin fee collection and withdrawal

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(unexpected_cfgs)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::needless_pass_by_value)] // Anchor handlers must take owned Context by design
#![allow(clippy::unnecessary_wraps)] // Anchor handlers return Result<()> for consistency
#![allow(deprecated)] // Anchor framework uses deprecated AccountInfo::realloc internally

use anchor_lang::prelude::*;

mod accept_authority;
mod admin_withdraw_fees;
mod cancel_authority_transfer;
mod close_agreement;
pub mod constants;
mod create_payment_terms;
pub mod errors;
pub mod events;
mod execute_payment;
mod init_config;
mod init_payee;
mod pause;
mod pause_agreement;
mod start_agreement;
pub mod state;
mod transfer_authority;
mod unpause;
mod update_config;
pub mod utils;

use accept_authority::*;
use admin_withdraw_fees::*;
use cancel_authority_transfer::*;
use close_agreement::*;
use create_payment_terms::*;
use execute_payment::*;
use init_config::*;
use init_payee::*;
use pause::*;
use pause_agreement::*;
use start_agreement::*;
use transfer_authority::*;
use unpause::*;
use update_config::*;

// Program ID is loaded from TALLY_PROGRAM_ID environment variable at compile time
// The build script (build.rs) converts the base58 program ID to bytes
// This approach ensures the program ID comes from the environment while satisfying
// Anchor's requirement for a compile-time constant in declare_id!()
const PROGRAM_ID_BYTES: &[u8; 32] = include_bytes!(concat!(env!("OUT_DIR"), "/program_id.bin"));
declare_id!(Pubkey::new_from_array(*PROGRAM_ID_BYTES));

#[program]
pub mod tally_protocol {
    use super::*;

    /// Initialize global program configuration
    ///
    /// # Errors
    /// Returns an error if:
    /// - The config account already exists
    /// - Account creation or initialization fails
    pub fn init_config(ctx: Context<InitConfig>, args: InitConfigArgs) -> Result<()> {
        init_config::handler(ctx, args)
    }

    /// Initialize a new payee account with USDC treasury and fee configuration
    ///
    /// # Errors
    /// Returns an error if:
    /// - The payee account already exists
    /// - Invalid USDC mint address
    /// - Fee configuration exceeds maximum allowed (10,000 basis points)
    /// - Account creation or initialization fails
    pub fn init_payee(ctx: Context<InitPayee>, args: InitPayeeArgs) -> Result<()> {
        init_payee::handler(ctx, args)
    }

    /// Create new payment terms for a payee
    ///
    /// # Errors
    /// Returns an error if:
    /// - Payment terms ID already exists for this payee
    /// - Price is zero or exceeds maximum
    /// - Period is invalid (too short or too long)
    /// - Grace period exceeds the period duration
    /// - Account creation fails
    pub fn create_payment_terms(ctx: Context<CreatePaymentTerms>, args: CreatePaymentTermsArgs) -> Result<()> {
        create_payment_terms::handler(ctx, args)
    }

    /// Start a new payment agreement for a user with delegate approval
    ///
    /// # Errors
    /// Returns an error if:
    /// - Payment agreement already exists for this user and payment terms
    /// - Insufficient USDC balance in user's account
    /// - Token transfer operations fail
    /// - Delegate approval amount is insufficient
    /// - Payment terms are inactive or expired
    /// - Account creation fails
    pub fn start_agreement(
        ctx: Context<StartAgreement>,
        args: StartAgreementArgs,
    ) -> Result<()> {
        start_agreement::handler(ctx, args)
    }

    /// Execute a payment for an existing agreement by pulling funds via delegate
    ///
    /// # Errors
    /// Returns an error if:
    /// - Payment agreement is not active or has been paused
    /// - Payment is not yet due (before `next_renewal_ts`)
    /// - Insufficient USDC balance for payment
    /// - Token transfer operations fail
    /// - Payment agreement has exceeded grace period
    /// - Delegate approval is insufficient or revoked
    pub fn execute_payment(
        ctx: Context<ExecutePayment>,
        args: ExecutePaymentArgs,
    ) -> Result<()> {
        execute_payment::handler(ctx, args)
    }

    /// Pause a payment agreement and revoke delegate approval
    ///
    /// # Errors
    /// Returns an error if:
    /// - Payment agreement does not exist or is already paused
    /// - Unauthorized pause attempt (wrong payer)
    /// - Token revoke operation fails
    /// - Account update operations fail
    pub fn pause_agreement(
        ctx: Context<PauseAgreement>,
        args: PauseAgreementArgs,
    ) -> Result<()> {
        pause_agreement::handler(ctx, args)
    }

    /// Close a paused payment agreement account and reclaim rent
    ///
    /// This instruction allows payers to close their payment agreement accounts
    /// after pausing to reclaim the rent (~0.00099792 SOL). The payment agreement
    /// must be inactive (paused) before it can be closed.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Payment agreement is still active (must be paused first)
    /// - Unauthorized closure attempt (wrong payer)
    /// - Payment agreement does not exist or is invalid
    /// - Account closure operations fail
    pub fn close_agreement(
        ctx: Context<CloseAgreement>,
        args: CloseAgreementArgs,
    ) -> Result<()> {
        close_agreement::handler(ctx, args)
    }

    /// Admin function to withdraw accumulated platform fees
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not authorized as platform admin
    /// - Insufficient fee balance to withdraw
    /// - Token transfer operations fail
    /// - Invalid withdrawal amount (zero or exceeds balance)
    pub fn admin_withdraw_fees(
        ctx: Context<AdminWithdrawFees>,
        args: AdminWithdrawFeesArgs,
    ) -> Result<()> {
        admin_withdraw_fees::handler(ctx, args)
    }

    /// Initiate platform authority transfer
    ///
    /// This begins a two-step authority transfer process. The current platform
    /// authority proposes a new authority, which must then accept the transfer.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the current platform authority
    /// - A pending transfer already exists
    /// - New authority is the same as current authority
    pub fn transfer_authority(
        ctx: Context<TransferAuthority>,
        args: TransferAuthorityArgs,
    ) -> Result<()> {
        transfer_authority::handler(ctx, args)
    }

    /// Accept platform authority transfer
    ///
    /// This completes a two-step authority transfer process. The new authority
    /// must sign to accept the transfer initiated by the current authority.
    ///
    /// # Errors
    /// Returns an error if:
    /// - No pending transfer exists
    /// - Caller is not the pending authority
    pub fn accept_authority(
        ctx: Context<AcceptAuthority>,
        args: AcceptAuthorityArgs,
    ) -> Result<()> {
        accept_authority::handler(ctx, args)
    }

    /// Cancel platform authority transfer
    ///
    /// This allows the current platform authority to cancel a pending authority
    /// transfer. This is useful if the transfer was initiated in error or if
    /// circumstances have changed.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the current platform authority
    /// - No pending transfer exists to cancel
    pub fn cancel_authority_transfer(
        ctx: Context<CancelAuthorityTransfer>,
        args: CancelAuthorityTransferArgs,
    ) -> Result<()> {
        cancel_authority_transfer::handler(ctx, args)
    }

    /// Pause the program
    ///
    /// This enables the emergency pause mechanism, disabling all user-facing operations
    /// (`start_agreement`, `execute_payment`, `create_payment_terms`) while allowing admin
    /// operations to continue for emergency fund recovery.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the platform authority
    pub fn pause(ctx: Context<Pause>, args: PauseArgs) -> Result<()> {
        pause::handler(ctx, args)
    }

    /// Unpause the program
    ///
    /// This disables the emergency pause mechanism, re-enabling all user-facing operations
    /// (`start_agreement`, `execute_payment`, `create_payment_terms`).
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the platform authority
    pub fn unpause(ctx: Context<Unpause>, args: UnpauseArgs) -> Result<()> {
        unpause::handler(ctx, args)
    }

    /// Update global configuration parameters
    ///
    /// This allows the platform authority to update global configuration parameters
    /// at runtime without redeploying the program. All changes take effect immediately.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the platform authority
    /// - `keeper_fee_bps` exceeds 100 (1%)
    /// - `min_platform_fee_bps` > `max_platform_fee_bps`
    /// - Any value is zero where positive values are required
    /// - No fields are provided for update
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        args: UpdateConfigArgs,
    ) -> Result<()> {
        update_config::handler(ctx, args)
    }

    /// Update subscription plan pricing and terms
    ///
    /// Allows the merchant authority to update an existing plan's price, period,
    /// grace period, and name without creating a new plan. This is useful for
    /// adjusting pricing, extending or shortening billing cycles, or updating
    /// plan details based on market conditions.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Caller is not the merchant authority
    /// - No fields are provided for update (at least one required)
    /// - New price is zero or exceeds maximum
    /// - New period is below minimum period from config
    /// - New grace period exceeds period or config maximum
    /// - New name is empty
    pub fn update_plan_terms(
        ctx: Context<UpdatePlanTerms>,
        args: UpdatePlanTermsArgs,
    ) -> Result<()> {
        update_plan_terms::handler(ctx, args)
    }
}
