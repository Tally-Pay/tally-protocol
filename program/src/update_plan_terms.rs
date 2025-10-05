use crate::constants::MAX_PLAN_PRICE_USDC;
use crate::errors::SubscriptionError;
use crate::events::PlanTermsUpdated;
use crate::state::{Config, Merchant, Plan};
use anchor_lang::prelude::*;

/// Arguments for updating a subscription plan's pricing and terms
///
/// All fields are optional - at least one must be provided.
/// Only the merchant authority can update plan terms.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct UpdatePlanTermsArgs {
    /// Price in USDC microlamports (6 decimals)
    /// Must be > 0 if provided
    pub price_usdc: Option<u64>,
    /// Subscription period in seconds
    /// Must be >= `config.min_period_seconds` if provided
    pub period_secs: Option<u64>,
    /// Grace period for renewals in seconds
    /// Must be <= period AND <= `config.max_grace_period_seconds` if provided
    pub grace_secs: Option<u64>,
    /// Plan display name
    /// Must not be empty if provided
    pub name: Option<String>,
}

#[derive(Accounts)]
pub struct UpdatePlanTerms<'info> {
    /// Global configuration account for validation
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// Plan account to update
    #[account(
        mut,
        seeds = [b"plan", merchant.key().as_ref(), plan.plan_id.as_ref()],
        bump,
        has_one = merchant
    )]
    pub plan: Account<'info, Plan>,

    /// Merchant account that owns the plan
    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    /// Authority signing the transaction (must be merchant authority)
    pub authority: Signer<'info>,
}

/// Converts a string to a fixed-size [u8; 32] array
/// Returns an error if the string's byte representation exceeds 32 bytes
/// Pads with zeros if the string is shorter than 32 bytes
fn string_to_bytes32(input: &str) -> Result<[u8; 32]> {
    let bytes = input.as_bytes();

    // Validate that byte length does not exceed 32
    require!(bytes.len() <= 32, SubscriptionError::InvalidPlan);

    let mut result = [0u8; 32];
    result[..bytes.len()].copy_from_slice(bytes);
    Ok(result)
}

pub fn handler(ctx: Context<UpdatePlanTerms>, args: UpdatePlanTermsArgs) -> Result<()> {
    let plan = &mut ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;
    let config = &ctx.accounts.config;

    // Validate authority: only merchant authority can update plan terms
    require!(
        ctx.accounts.authority.key() == merchant.authority,
        SubscriptionError::Unauthorized
    );

    // Require at least one field to be updated
    require!(
        args.price_usdc.is_some()
            || args.period_secs.is_some()
            || args.grace_secs.is_some()
            || args.name.is_some(),
        SubscriptionError::InvalidPlan
    );

    // Store old values for event emission
    let old_price = plan.price_usdc;
    let old_period = plan.period_secs;
    let old_grace = plan.grace_secs;

    // Track which fields were updated for event emission
    let mut price_updated = false;
    let mut period_updated = false;
    let mut grace_updated = false;

    // Update price if provided
    if let Some(new_price) = args.price_usdc {
        // Validate price > 0
        require!(new_price > 0, SubscriptionError::InvalidPlan);

        // Validate price <= MAX_PLAN_PRICE_USDC
        require!(
            new_price <= MAX_PLAN_PRICE_USDC,
            SubscriptionError::InvalidPlan
        );

        plan.price_usdc = new_price;
        price_updated = true;
    }

    // Update period if provided
    if let Some(new_period) = args.period_secs {
        // Validate period >= minimum period from config
        require!(
            new_period >= config.min_period_seconds,
            SubscriptionError::InvalidPlan
        );

        plan.period_secs = new_period;
        period_updated = true;
    }

    // Update grace period if provided
    if let Some(new_grace) = args.grace_secs {
        // Use the updated period if it was changed, otherwise use existing period
        let effective_period = if period_updated {
            plan.period_secs
        } else {
            old_period
        };

        // Validate grace_secs <= 30% of period_secs
        let max_grace_period = effective_period
            .checked_mul(3)
            .and_then(|v| v.checked_div(10))
            .ok_or(SubscriptionError::ArithmeticError)?;

        require!(new_grace <= max_grace_period, SubscriptionError::InvalidPlan);

        // Validate grace_secs <= max_grace_period_seconds from config
        require!(
            new_grace <= config.max_grace_period_seconds,
            SubscriptionError::InvalidPlan
        );

        plan.grace_secs = new_grace;
        grace_updated = true;
    }

    // Update name if provided
    if let Some(new_name) = &args.name {
        // Validate name is not empty
        require!(!new_name.is_empty(), SubscriptionError::InvalidPlan);

        // Convert and validate name byte length <= 32
        let name_bytes = string_to_bytes32(new_name)?;
        plan.name = name_bytes;
    }

    // Emit PlanTermsUpdated event
    emit!(PlanTermsUpdated {
        plan: plan.key(),
        merchant: merchant.key(),
        old_price: if price_updated { Some(old_price) } else { None },
        new_price: args.price_usdc,
        old_period: if period_updated { Some(old_period) } else { None },
        new_period: args.period_secs,
        old_grace: if grace_updated { Some(old_grace) } else { None },
        new_grace: args.grace_secs,
        updated_by: ctx.accounts.authority.key(),
    });

    msg!(
        "Plan terms updated: plan={}, price_updated={}, period_updated={}, grace_updated={}, name_updated={}",
        plan.key(),
        price_updated,
        period_updated,
        grace_updated,
        args.name.is_some()
    );

    Ok(())
}
