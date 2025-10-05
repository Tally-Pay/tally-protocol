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
/// Returns an error if the string's byte representation exceeds 32 bytes
/// Pads with zeros if the string is shorter than 32 bytes
fn string_to_bytes32(input: &str) -> Result<[u8; 32]> {
    let bytes = input.as_bytes();

    // Validate that byte length does not exceed 32
    // This prevents silent truncation which could cause collisions
    require!(
        bytes.len() <= 32,
        SubscriptionError::InvalidPlan
    );

    let mut result = [0u8; 32];
    result[..bytes.len()].copy_from_slice(bytes);
    Ok(result)
}

pub fn handler(ctx: Context<CreatePlan>, args: CreatePlanArgs) -> Result<()> {
    // Validate price_usdc > 0
    require!(args.price_usdc > 0, SubscriptionError::InvalidPlan);

    // Validate period_secs >= minimum period from config
    require!(
        args.period_secs >= ctx.accounts.config.min_period_seconds,
        SubscriptionError::InvalidPlan
    );

    // Validate grace_secs <= 30% of period_secs (L-2 security fix)
    //
    // Limiting grace periods to 30% of the subscription period prevents merchants from
    // creating plans where subscribers can effectively delay payment for the entire
    // subscription duration. This reduces merchant payment risk and aligns with standard
    // subscription practices where grace periods should be a reasonable fraction of the
    // billing cycle, not equal to or exceeding it.
    //
    // Example scenarios:
    // - Monthly subscription (30 days): Maximum 9-day grace period
    // - Weekly subscription (7 days): Maximum 2-day grace period
    // - Annual subscription (365 days): Maximum 109-day grace period
    //
    // Note: This validation uses integer division (period_secs * 3 / 10) which rounds
    // down, ensuring conservative grace period limits.
    //
    // Security: Using checked arithmetic to prevent overflow. If overflow occurs,
    // the validation fails as expected (grace period would be invalid).
    let max_grace_period = args
        .period_secs
        .checked_mul(3)
        .and_then(|v| v.checked_div(10))
        .ok_or(SubscriptionError::ArithmeticError)?;

    require!(
        args.grace_secs <= max_grace_period,
        SubscriptionError::InvalidPlan
    );

    // Validate grace_secs <= max_grace_period_seconds from config
    // This enforces an absolute maximum to prevent extreme cases (e.g., multi-year grace periods)
    require!(
        args.grace_secs <= ctx.accounts.config.max_grace_period_seconds,
        SubscriptionError::InvalidPlan
    );

    // Validate plan_id is not empty
    require!(
        !args.plan_id.is_empty(),
        SubscriptionError::InvalidPlan
    );

    // Validate name is not empty
    require!(
        !args.name.is_empty(),
        SubscriptionError::InvalidPlan
    );

    // Validate that plan_id_bytes matches the string conversion to ensure consistency
    // This also validates that plan_id byte length <= 32
    let expected_plan_id_bytes = string_to_bytes32(&args.plan_id)?;
    require!(
        args.plan_id_bytes == expected_plan_id_bytes,
        SubscriptionError::InvalidPlan
    );

    // Convert and validate name byte length <= 32
    let name_bytes = string_to_bytes32(&args.name)?;

    // Defense-in-depth: Explicitly verify the plan account has not been initialized
    // While the `init` constraint already prevents duplicate creation, this check
    // provides an additional safety layer against potential PDA collisions or framework issues
    let plan_account_info = ctx.accounts.plan.to_account_info();
    require!(
        plan_account_info.data_is_empty(),
        SubscriptionError::PlanAlreadyExists
    );

    let plan = &mut ctx.accounts.plan;
    plan.merchant = ctx.accounts.merchant.key();
    plan.plan_id = args.plan_id_bytes; // Use the validated plan_id_bytes directly
    plan.price_usdc = args.price_usdc;
    plan.period_secs = args.period_secs;
    plan.grace_secs = args.grace_secs;
    plan.name = name_bytes;
    plan.active = true; // Set active = true by default

    // Get current timestamp for event
    let clock = Clock::get()?;

    // Emit PlanCreated event
    emit!(crate::events::PlanCreated {
        plan: plan.key(),
        merchant: ctx.accounts.merchant.key(),
        plan_id: args.plan_id,
        price_usdc: args.price_usdc,
        period_secs: args.period_secs,
        grace_secs: args.grace_secs,
        name: args.name,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
