use crate::constants::MAX_PLAN_PRICE_USDC;
use crate::errors::SubscriptionError;
use crate::state::{Merchant, Plan};
use anchor_lang::prelude::*;

/// Arguments for creating a subscription plan.
///
/// # Rate Limiting Considerations
///
/// This instruction has **no on-chain rate limiting** by design. Spam prevention is handled
/// through economic incentives and off-chain monitoring:
///
/// ## Economic Deterrence
/// - **Transaction Fee**: 0.000005 SOL (~$0.0007) per plan creation
/// - **Rent Deposit**: 0.00089 SOL (~$0.12) per plan (129 bytes account size)
/// - **Total Cost**: ~$0.12 per plan, making mass spam attacks expensive
///
/// Example: Creating 10,000 fake plans costs ~$1,253 (rent + fees), which provides
/// natural spam deterrence without requiring on-chain rate limiting logic.
///
/// ## Off-Chain Monitoring
///
/// Recommended monitoring thresholds (see `/docs/SPAM_DETECTION.md`):
/// - **Critical Alert**: >100 plans created per merchant per hour
/// - **Warning Alert**: >10 plans created per merchant per hour
/// - **RPC Rate Limit**: Throttle to 10 plan creations per merchant per hour
///
/// ## Why No On-Chain Rate Limiting?
///
/// On-chain rate limiting was deliberately not implemented because:
/// 1. **Account Migration Complexity**: Adding rate limit fields (timestamps, counters) to
///    `Merchant` accounts requires migrating all existing accounts, risking data loss.
/// 2. **Storage Costs**: Rate limit state increases account size and rent costs for all merchants.
/// 3. **Solana Best Practices**: Rate limiting is more effectively handled at the RPC and
///    indexer layers where it's flexible, configurable, and doesn't bloat on-chain state.
/// 4. **Economic Model**: Transaction fees + rent deposits already provide spam deterrence.
///
/// For comprehensive rate limiting strategy, see `/docs/RATE_LIMITING_STRATEGY.md`.
///
/// ## Mitigation Recommendations
///
/// 1. **RPC Layer**: Configure rate limits (e.g., Nginx, `HAProxy`, or RPC provider limits)
/// 2. **Indexer**: Deploy spam detection indexer to monitor merchant activity patterns
/// 3. **Dashboard**: Real-time alerting for anomalous plan creation rates
/// 4. **Auto-Throttle**: Implement automatic account throttling for detected spam patterns
///
/// See `/docs/OPERATIONAL_PROCEDURES.md` for incident response procedures.
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
        bump = config.bump,
        constraint = !config.paused @ SubscriptionError::Inactive
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

    // Validate price_usdc <= MAX_PLAN_PRICE_USDC (M-5 security fix)
    //
    // Enforces a maximum price limit to prevent social engineering attacks where merchants
    // create plans with extreme prices (e.g., u64::MAX) that could mislead subscribers.
    //
    // Security Impact:
    // - Prevents creation of plans with prices near u64::MAX (~18.4 quintillion USDC)
    // - Mitigates social engineering risks from unrealistic price displays
    // - Reduces potential overflow scenarios in downstream calculations
    // - Establishes reasonable ceiling (1 million USDC) for subscription services
    //
    // This validation ensures that all plan prices remain within a realistic range
    // suitable for legitimate subscription business models while blocking extreme
    // values that have no valid use case and could enable malicious behavior.
    require!(
        args.price_usdc <= MAX_PLAN_PRICE_USDC,
        SubscriptionError::InvalidPlan
    );

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
    // Integer Division Behavior (L-6 Audit Finding - Acceptable by Design):
    // This validation intentionally uses integer division (period_secs * 3 / 10) which
    // rounds down, creating conservative grace period limits. For periods not divisible
    // by 10, the maximum grace is slightly less than 30%.
    //
    // Examples of integer division rounding:
    // - period = 11s → max_grace = 3s (not 3.3s, rounds down by 0.3s)
    // - period = 101s → max_grace = 30s (not 30.3s, rounds down by 0.3s)
    // - period = 2,851,201s → max_grace = 855,360s (30% - 0.3s, negligible variance)
    //
    // Why floor division (rounding down) is the correct choice:
    // 1. Conservative Security: Rounding down provides an additional safety margin,
    //    ensuring grace periods never exceed the intended 30% limit.
    // 2. Predictable Behavior: Floor division is deterministic and matches Rust's
    //    standard integer division semantics.
    // 3. Negligible Impact: For real-world subscription periods (hours, days, weeks,
    //    months), the sub-second rounding difference is negligible.
    //
    // Why ceiling division (rounding up) is NOT recommended:
    // 1. Security Risk: Would allow grace periods to exceed 30% for certain values,
    //    violating the security requirement.
    // 2. Unpredictable Edge Cases: Could create inconsistent behavior for merchants
    //    setting similar subscription periods.
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
