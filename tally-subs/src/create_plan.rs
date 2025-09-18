use crate::errors::SubscriptionError;
use crate::state::{Merchant, Plan};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreatePlanArgs {
    pub plan_id: String,         // Original string plan ID
    pub plan_id_bytes: [u8; 32], // Padded plan_id bytes for PDA seeds (must match SDK calculation)
    pub price_usdc: u64,         // Price in USDC microlamports
    pub period_secs: u64,        // Subscription period in seconds
    pub grace_secs: u64,         // Grace period for renewals
    pub name: String,            // Plan name, will be converted to [u8; 32]
}

#[derive(Accounts)]
#[instruction(args: CreatePlanArgs)]
pub struct CreatePlan<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(
        init,
        payer = authority,
        space = Plan::SPACE,
        seeds = [b"plan", merchant.key().as_ref(), args.plan_id_bytes.as_ref()],
        bump
    )]
    pub plan: Account<'info, Plan>,

    #[account(
        seeds = [b"merchant", authority.key().as_ref()],
        bump = merchant.bump,
        has_one = authority
    )]
    pub merchant: Account<'info, Merchant>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Converts a string to a fixed-size [u8; 32] array
/// Truncates if longer than 32 bytes, pads with zeros if shorter
fn string_to_bytes32(input: &str) -> [u8; 32] {
    let mut result = [0u8; 32];
    let bytes = input.as_bytes();
    let copy_len = std::cmp::min(bytes.len(), 32);
    result[..copy_len].copy_from_slice(&bytes[..copy_len]);
    result
}

pub fn handler(ctx: Context<CreatePlan>, args: CreatePlanArgs) -> Result<()> {
    // Validate price_usdc > 0
    require!(args.price_usdc > 0, SubscriptionError::InvalidPlan);

    // Validate period_secs >= minimum period from config
    require!(
        args.period_secs >= ctx.accounts.config.min_period_seconds,
        SubscriptionError::InvalidPlan
    );

    // Validate grace_secs <= 2 * period_secs
    require!(
        args.grace_secs
            <= args
                .period_secs
                .checked_mul(2)
                .ok_or(SubscriptionError::ArithmeticError)?,
        SubscriptionError::InvalidPlan
    );

    // Validate plan_id is not empty and within reasonable length
    require!(
        !args.plan_id.is_empty() && args.plan_id.len() <= 32,
        SubscriptionError::InvalidPlan
    );

    // Validate name is not empty and within reasonable length
    require!(
        !args.name.is_empty() && args.name.len() <= 32,
        SubscriptionError::InvalidPlan
    );

    // Validate that plan_id_bytes matches the string conversion to ensure consistency
    let expected_plan_id_bytes = string_to_bytes32(&args.plan_id);
    require!(
        args.plan_id_bytes == expected_plan_id_bytes,
        SubscriptionError::InvalidPlan
    );

    let plan = &mut ctx.accounts.plan;
    plan.merchant = ctx.accounts.merchant.key();
    plan.plan_id = args.plan_id_bytes; // Use the validated plan_id_bytes directly
    plan.price_usdc = args.price_usdc;
    plan.period_secs = args.period_secs;
    plan.grace_secs = args.grace_secs;
    plan.name = string_to_bytes32(&args.name);
    plan.active = true; // Set active = true by default

    msg!(
        "Created plan: id={}, price={}, period={}, grace={}, active={}",
        args.plan_id,
        args.price_usdc,
        args.period_secs,
        args.grace_secs,
        plan.active
    );

    Ok(())
}
