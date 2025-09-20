//! Solana Subscriptions Program
//!
//! A Solana-native subscription platform implementing delegate-based USDC payments.
//! This program enables merchants to create subscription plans and collect recurring
//! payments through SPL Token delegate approvals, eliminating the need for user
//! signatures on each renewal.
//!
//! ## Core Features
//! - Merchant registration with fee configuration
//! - Subscription plan creation with flexible pricing and periods
//! - Delegate-based recurring payments using USDC
//! - Automatic subscription renewals via off-chain keeper
//! - Grace period handling for failed payments
//! - Admin fee collection and withdrawal

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(unexpected_cfgs)]
#![allow(clippy::wildcard_imports)] // TODO: Fix specific imports in future
#![allow(clippy::needless_pass_by_value)] // Anchor handlers must take owned Context by design
#![allow(clippy::unnecessary_wraps)] // Anchor handlers return Result<()> for consistency
#![allow(deprecated)] // Anchor framework uses deprecated AccountInfo::realloc internally

use anchor_lang::prelude::*;

mod admin_withdraw_fees;
mod cancel_subscription;
mod create_plan;
pub mod errors;
pub mod events;
mod init_config;
mod init_merchant;
mod renew_subscription;
mod start_subscription;
pub mod state;

use admin_withdraw_fees::*;
use cancel_subscription::*;
use create_plan::*;
use init_config::*;
use init_merchant::*;
use renew_subscription::*;
use start_subscription::*;

declare_id!("Fwrs8tRRtw8HwmQZFS3XRRVcKBQhe1nuZ5heB4FgySXV");

#[program]
pub mod subs {
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

    /// Initialize a new merchant account with USDC treasury and fee configuration
    ///
    /// # Errors
    /// Returns an error if:
    /// - The merchant account already exists
    /// - Invalid USDC mint address
    /// - Fee configuration exceeds maximum allowed (10,000 basis points)
    /// - Account creation or initialization fails
    pub fn init_merchant(ctx: Context<InitMerchant>, args: InitMerchantArgs) -> Result<()> {
        init_merchant::handler(ctx, args)
    }

    /// Create a new subscription plan for a merchant
    ///
    /// # Errors
    /// Returns an error if:
    /// - Plan ID already exists for this merchant
    /// - Price is zero or exceeds maximum
    /// - Period is invalid (too short or too long)
    /// - Grace period exceeds the period duration
    /// - Account creation fails
    pub fn create_plan(ctx: Context<CreatePlan>, args: CreatePlanArgs) -> Result<()> {
        create_plan::handler(ctx, args)
    }

    /// Start a new subscription for a user with delegate approval
    ///
    /// # Errors
    /// Returns an error if:
    /// - Subscription already exists for this user and plan
    /// - Insufficient USDC balance in user's account
    /// - Token transfer operations fail
    /// - Delegate approval amount is insufficient
    /// - Plan is inactive or expired
    /// - Account creation fails
    pub fn start_subscription(
        ctx: Context<StartSubscription>,
        args: StartSubscriptionArgs,
    ) -> Result<()> {
        start_subscription::handler(ctx, args)
    }

    /// Renew an existing subscription by pulling funds via delegate
    ///
    /// # Errors
    /// Returns an error if:
    /// - Subscription is not active or has been cancelled
    /// - Renewal is not yet due (before next_renewal_ts)
    /// - Insufficient USDC balance for renewal
    /// - Token transfer operations fail
    /// - Subscription has exceeded grace period
    /// - Delegate approval is insufficient or revoked
    pub fn renew_subscription(
        ctx: Context<RenewSubscription>,
        args: RenewSubscriptionArgs,
    ) -> Result<()> {
        renew_subscription::handler(ctx, args)
    }

    /// Cancel a subscription and revoke delegate approval
    ///
    /// # Errors
    /// Returns an error if:
    /// - Subscription does not exist or is already cancelled
    /// - Unauthorized cancellation attempt (wrong subscriber)
    /// - Token revoke operation fails
    /// - Account update operations fail
    pub fn cancel_subscription(
        ctx: Context<CancelSubscription>,
        args: CancelSubscriptionArgs,
    ) -> Result<()> {
        cancel_subscription::handler(ctx, args)
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
}
