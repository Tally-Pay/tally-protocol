use crate::{
    constants::{
        FEE_BASIS_POINTS_DIVISOR, TRIAL_DURATION_14_DAYS, TRIAL_DURATION_30_DAYS,
        TRIAL_DURATION_7_DAYS,
    },
    errors::SubscriptionError,
    events::*,
    state::*,
    utils::validate_platform_treasury,
};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

/// Arguments for starting a new subscription or reactivating a canceled subscription.
///
/// # Rate Limiting Considerations
///
/// This instruction has **no on-chain rate limiting** by design. Spam prevention relies on
/// economic costs and off-chain monitoring:
///
/// ## Economic Deterrence
/// - **Transaction Fee**: 0.000005 SOL (~$0.0007) per subscription start
/// - **Rent Deposit**: 0.00078 SOL (~$0.11) per new subscription (110 bytes account size)
/// - **USDC Payment**: Requires actual USDC transfer for initial payment
/// - **Delegate Approval**: Requires pre-approval of USDC token delegate
///
/// **Subscription Churn Attack Cost**: Repeatedly starting and canceling the same subscription:
/// - Per cycle: 0.00001 SOL (start + cancel) + USDC for initial payment
/// - 1,000 cycles: ~$2-5 (depending on USDC amount and gas)
///
/// The USDC payment requirement and delegate setup add friction that makes high-frequency
/// churn attacks more complex than simple account creation spam.
///
/// ## Off-Chain Monitoring
///
/// Recommended monitoring thresholds (see `/docs/SPAM_DETECTION.md`):
/// - **Churn Alert**: >80% of subscriptions canceled within 1 hour of starting
/// - **Volume Alert**: >20 subscription operations (start/cancel) per account per hour
/// - **Pattern Alert**: Rapid start-cancel cycles on same plan/subscriber pairs
///
/// ## Attack Scenarios
///
/// ### Subscription Churn Attack
/// **Attack**: User repeatedly starts and cancels same subscription to generate event noise.
/// **Cost**: Low (~$0.002 per cycle), but requires USDC balance and delegate approval.
/// **Detection**: Monitor subscription lifetime duration; flag subscriptions lasting <5 minutes.
/// **Mitigation**: RPC rate limiting to 20 subscription operations per hour per account.
///
/// ### Reactivation Spam
/// **Attack**: User exploits `init_if_needed` to repeatedly reactivate canceled subscriptions.
/// **Cost**: 0.000005 SOL per reactivation (no rent deposit after first creation).
/// **Detection**: Track reactivation frequency; alert on >5 reactivations per hour.
/// **Mitigation**: Application-layer cooldown period between cancellation and reactivation.
///
/// ## Why No On-Chain Rate Limiting?
///
/// On-chain rate limiting was deliberately not implemented because:
/// 1. **Account Complexity**: Adding rate limit fields to `Subscription` accounts increases
///    complexity and storage costs for all users.
/// 2. **State Bloat**: Tracking per-subscriber operation timestamps bloats on-chain state.
/// 3. **Flexibility**: Off-chain rate limits can be adjusted dynamically based on observed
///    attack patterns without requiring program upgrades.
/// 4. **Economic Model**: USDC payment requirement provides natural spam deterrence.
///
/// For comprehensive rate limiting strategy, see `/docs/RATE_LIMITING_STRATEGY.md`.
///
/// ## Mitigation Recommendations
///
/// 1. **RPC Layer**: Limit subscription operations to 20/hour per account
/// 2. **Indexer**: Monitor subscription lifetime and churn patterns
/// 3. **Application Layer**: Implement cooldown periods between cancel and reactivation
/// 4. **Dashboard**: Alert on abnormal subscription churn rates
///
/// See `/docs/OPERATIONAL_PROCEDURES.md` for incident response procedures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct StartSubscriptionArgs {
    pub allowance_periods: u8,       // Multiplier for allowance (default 3)
    pub trial_duration_secs: Option<u64>, // Optional trial period: 7, 14, or 30 days (in seconds)
}

#[derive(Accounts)]
pub struct StartSubscription<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump,
        constraint = !config.paused @ SubscriptionError::Inactive
    )]
    pub config: Account<'info, Config>,

    #[account(
        init_if_needed,
        payer = subscriber,
        space = Subscription::SPACE,
        seeds = [b"subscription", plan.key().as_ref(), subscriber.key().as_ref()],
        bump
    )]
    pub subscription: Account<'info, Subscription>,

    #[account(
        constraint = plan.active @ SubscriptionError::Inactive
    )]
    pub plan: Account<'info, Plan>,

    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    #[account(mut)]
    pub subscriber: Signer<'info>,

    // USDC accounts for transfers
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub subscriber_usdc_ata: UncheckedAccount<'info>,

    /// CHECK: Validated as merchant treasury ATA in handler
    #[account(mut)]
    pub merchant_treasury_ata: UncheckedAccount<'info>,

    /// CHECK: Validated as platform treasury ATA in handler
    #[account(mut)]
    pub platform_treasury_ata: UncheckedAccount<'info>,

    /// CHECK: Validated as USDC mint in handler
    pub usdc_mint: UncheckedAccount<'info>,

    // Program PDA that acts as delegate
    /// CHECK: PDA derived from program, validated in handler
    #[account(
        seeds = [b"delegate"],
        bump
    )]
    pub program_delegate: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[allow(clippy::too_many_lines)]
pub fn handler(ctx: Context<StartSubscription>, args: StartSubscriptionArgs) -> Result<()> {
    let subscription = &mut ctx.accounts.subscription;
    let plan = &ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;

    // Detect if this is reactivation (account already exists) vs new subscription
    // created_ts will be non-zero for existing accounts since it's set during initialization
    let is_reactivation = subscription.created_ts != 0;

    if is_reactivation {
        // REACTIVATION PATH: Validate and reactivate existing subscription

        // Security check: Prevent reactivation if already active
        require!(!subscription.active, SubscriptionError::AlreadyActive);

        // Security check: Ensure plan and subscriber match (prevent account hijacking)
        require!(
            subscription.plan == plan.key(),
            SubscriptionError::Unauthorized
        );
        require!(
            subscription.subscriber == ctx.accounts.subscriber.key(),
            SubscriptionError::Unauthorized
        );

        // Trial abuse prevention: Trials only allowed for new subscriptions, not reactivations
        // This prevents users from repeatedly canceling and reactivating to get multiple trials
        if args.trial_duration_secs.is_some() {
            return Err(SubscriptionError::TrialAlreadyUsed.into());
        }
    } else {
        // NEW SUBSCRIPTION PATH: Validate trial parameters if provided
        if let Some(trial_secs) = args.trial_duration_secs {
            // Validate trial duration is exactly 7, 14, or 30 days
            if trial_secs != TRIAL_DURATION_7_DAYS
                && trial_secs != TRIAL_DURATION_14_DAYS
                && trial_secs != TRIAL_DURATION_30_DAYS
            {
                return Err(SubscriptionError::InvalidTrialDuration.into());
            }
        }
    }

    // Deserialize and validate token accounts with specific error handling
    let subscriber_ata_data: TokenAccount =
        TokenAccount::try_deserialize(&mut ctx.accounts.subscriber_usdc_ata.data.borrow().as_ref())
            .map_err(|_| SubscriptionError::InvalidSubscriberTokenAccount)?;

    let merchant_treasury_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.merchant_treasury_ata.data.borrow().as_ref(),
    )
    .map_err(|_| SubscriptionError::InvalidMerchantTreasuryAccount)?;

    let platform_treasury_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.platform_treasury_ata.data.borrow().as_ref(),
    )
    .map_err(|_| SubscriptionError::InvalidPlatformTreasuryAccount)?;

    let usdc_mint_data: Mint =
        Mint::try_deserialize(&mut ctx.accounts.usdc_mint.data.borrow().as_ref())
            .map_err(|_| SubscriptionError::InvalidUsdcMint)?;

    // Runtime validation: Ensure platform treasury ATA remains valid
    // This prevents denial-of-service if the platform authority closes or modifies
    // the treasury ATA after config initialization (audit finding L-4)
    validate_platform_treasury(
        &ctx.accounts.platform_treasury_ata,
        &ctx.accounts.config.platform_authority,
        &ctx.accounts.config.allowed_mint,
        &ctx.accounts.token_program,
    )?;

    // Validate token account ownership and mints
    if subscriber_ata_data.owner != ctx.accounts.subscriber.key() {
        return Err(SubscriptionError::Unauthorized.into());
    }

    if subscriber_ata_data.mint != merchant.usdc_mint
        || merchant_treasury_data.mint != merchant.usdc_mint
        || platform_treasury_data.mint != merchant.usdc_mint
    {
        return Err(SubscriptionError::WrongMint.into());
    }

    if ctx.accounts.usdc_mint.key() != merchant.usdc_mint {
        return Err(SubscriptionError::WrongMint.into());
    }

    if ctx.accounts.merchant_treasury_ata.key() != merchant.treasury_ata {
        return Err(SubscriptionError::BadSeeds.into());
    }

    // Use default from config if allowance_periods is 0
    let allowance_periods = if args.allowance_periods == 0 {
        ctx.accounts.config.default_allowance_periods
    } else {
        args.allowance_periods
    };

    // Get current time
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // Validate allowance calculation won't overflow
    // Ensure price_usdc * allowance_periods <= u64::MAX
    let allowance_periods_u64 = u64::from(allowance_periods);
    let max_safe_price = u64::MAX
        .checked_div(allowance_periods_u64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    require!(
        plan.price_usdc <= max_safe_price,
        SubscriptionError::InvalidPlan
    );

    // Determine if this is a trial subscription
    let is_trial = !is_reactivation && args.trial_duration_secs.is_some();

    // Validate delegate allowance for multi-period subscription start
    //
    // ALLOWANCE MANAGEMENT EXPECTATIONS (Audit L-3):
    //
    // For subscription initiation, we require allowance for multiple periods
    // (default 3x, configurable via allowance_periods parameter) to ensure
    // seamless renewals without immediate allowance exhaustion.
    //
    // IMPORTANT: Subsequent renewals check allowance >= plan.price_usdc (single period).
    // This design allows flexibility in allowance management while preventing immediate
    // renewal failures. Users should maintain sufficient allowance (recommended: 2x plan price)
    // to avoid renewal interruptions.
    //
    // The asymmetry is intentional:
    // - Start: Requires multi-period allowance to prevent immediate renewal failures
    // - Renewal: Requires single-period allowance, emits warning when low (< 2x price)
    //
    // Off-chain systems should monitor LowAllowanceWarning events to prompt users
    // to increase allowance before the next renewal cycle.
    //
    // TRIAL SUBSCRIPTIONS:
    // During trial periods, delegate approval is still required but no immediate payment
    // is made. The allowance is validated to ensure the first payment (when trial ends)
    // can be processed successfully.
    let required_allowance = plan
        .price_usdc
        .checked_mul(allowance_periods_u64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    if subscriber_ata_data.delegated_amount < required_allowance {
        return Err(SubscriptionError::InsufficientAllowance.into());
    }

    // Explicitly validate PDA derivation to ensure the delegate PDA was derived with expected seeds
    let (expected_delegate_pda, _expected_bump) =
        Pubkey::find_program_address(&[b"delegate"], ctx.program_id);
    require!(
        ctx.accounts.program_delegate.key() == expected_delegate_pda,
        SubscriptionError::BadSeeds
    );

    // Validate delegate is our program PDA
    //
    // GLOBAL DELEGATE ARCHITECTURE:
    //
    // This protocol uses a single global delegate PDA for all merchants and subscriptions.
    // SPL Token accounts support only ONE delegate at a time, but because all merchants
    // share the same global delegate, users can subscribe to MULTIPLE merchants using
    // the same token account without delegate conflicts.
    //
    // Security model:
    // - The global delegate PDA has no private key (only the program can sign with it)
    // - Program validation enforces that only valid subscriptions can be renewed
    // - Each subscription is bound to a specific plan via PDA derivation
    // - Merchants cannot renew each other's subscriptions (different subscription PDAs)
    // - Transfer amounts are validated against plan.price_usdc
    //
    // Benefits:
    // - Users can subscribe to multiple merchants with one token account
    // - Enables budget compartmentalization (dedicated subscription wallets)
    // - Supports hierarchical payment structures (company → departments → employees)
    // - Simplifies allowance management across multiple merchants
    let actual_delegate = Option::<Pubkey>::from(subscriber_ata_data.delegate);
    if actual_delegate != Some(expected_delegate_pda) {
        return Err(SubscriptionError::Unauthorized.into());
    }

    // PAYMENT PROCESSING: Skip during trial period
    //
    // Trial subscriptions do not require immediate payment. The delegate approval
    // has already been validated above, ensuring payment can be processed when
    // the trial ends. The first payment will occur during the renewal at trial_ends_at.
    if !is_trial {
        // Calculate platform fee using checked arithmetic
        let platform_fee = u64::try_from(
            u128::from(plan.price_usdc)
                .checked_mul(u128::from(merchant.platform_fee_bps))
                .ok_or(SubscriptionError::ArithmeticError)?
                .checked_div(FEE_BASIS_POINTS_DIVISOR)
                .ok_or(SubscriptionError::ArithmeticError)?,
        )
        .map_err(|_| SubscriptionError::ArithmeticError)?;

        let merchant_amount = plan
            .price_usdc
            .checked_sub(platform_fee)
            .ok_or(SubscriptionError::ArithmeticError)?;

        // Prepare delegate signer seeds
        let delegate_bump = ctx.bumps.program_delegate;
        let delegate_seeds: &[&[&[u8]]] =
            &[&[b"delegate", &[delegate_bump]]];

        // Get USDC mint decimals from the mint account
        let usdc_decimals = usdc_mint_data.decimals;

        // Transfer merchant amount to merchant treasury (via delegate)
        if merchant_amount > 0 {
            let transfer_to_merchant = TransferChecked {
                from: ctx.accounts.subscriber_usdc_ata.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.merchant_treasury_ata.to_account_info(),
                authority: ctx.accounts.program_delegate.to_account_info(),
            };

            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    transfer_to_merchant,
                    delegate_seeds,
                ),
                merchant_amount,
                usdc_decimals,
            )?;
        }

        // Transfer platform fee to platform treasury (via delegate)
        if platform_fee > 0 {
            let transfer_to_platform = TransferChecked {
                from: ctx.accounts.subscriber_usdc_ata.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.platform_treasury_ata.to_account_info(),
                authority: ctx.accounts.program_delegate.to_account_info(),
            };

            token::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    transfer_to_platform,
                    delegate_seeds,
                ),
                platform_fee,
                usdc_decimals,
            )?;
        }
    }

    // Calculate next renewal timestamp
    //
    // For trial subscriptions: next_renewal_ts = trial_ends_at (when trial period ends)
    // For paid subscriptions: next_renewal_ts = current_time + period_secs
    let next_renewal_ts = if is_trial {
        let trial_secs = args.trial_duration_secs
            .ok_or(SubscriptionError::InvalidTrialDuration)?;
        let trial_duration_i64 =
            i64::try_from(trial_secs).map_err(|_| SubscriptionError::ArithmeticError)?;
        current_time
            .checked_add(trial_duration_i64)
            .ok_or(SubscriptionError::ArithmeticError)?
    } else {
        let period_i64 =
            i64::try_from(plan.period_secs).map_err(|_| SubscriptionError::ArithmeticError)?;
        current_time
            .checked_add(period_i64)
            .ok_or(SubscriptionError::ArithmeticError)?
    };

    // Update subscription account based on whether this is new or reactivation
    if is_reactivation {
        // ============================================================================
        // REACTIVATION PATH: Preserve Historical Fields While Resetting Billing Cycle
        // ============================================================================
        //
        // When a previously canceled subscription is reactivated, we intentionally preserve
        // certain historical fields to maintain a complete record of the subscription's
        // lifetime across all sessions.
        //
        // This design supports:
        // - Loyalty programs based on lifetime subscription duration
        // - Analytics on long-term customer engagement
        // - Business intelligence on churn and reactivation patterns
        // - Tiered benefits based on cumulative renewals
        //
        // For comprehensive lifecycle documentation and off-chain integration patterns,
        // see docs/SUBSCRIPTION_LIFECYCLE.md
        //
        // ------------------------------------------------------------------------
        // PRESERVED FIELDS (Historical Record - Intentionally NOT Modified)
        // ------------------------------------------------------------------------
        //
        // 1. created_ts: Original subscription creation timestamp
        //    - Purpose: Track the start of the subscriber-merchant relationship
        //    - Example: A subscription created on 2024-01-01, canceled, and reactivated
        //              on 2024-06-01 will still show created_ts = 2024-01-01
        //    - Use Case: Calculate total relationship duration, anniversary rewards
        //
        // 2. renewals: Cumulative renewal count across ALL sessions
        //    - Purpose: Track total billing cycles across the subscription's lifetime
        //    - Behavior: Continues incrementing from previous session's count
        //    - Example: A subscription with 10 renewals, when reactivated, continues
        //              counting from 10 (next renewal will be 11, not 1)
        //    - Use Case: Lifetime value calculations, tier-based loyalty programs
        //    - Off-Chain: Systems tracking "current session renewals" must calculate:
        //                 current_session_renewals = total_renewals - renewals_at_session_start
        //                 See docs/SUBSCRIPTION_LIFECYCLE.md for indexer examples
        //
        // 3. bump: PDA derivation seed (immutable by design)
        //    - Purpose: Account address derivation parameter
        //    - Behavior: Never changes (fundamental property of the account)
        //
        // ------------------------------------------------------------------------
        // RESET FIELDS (New Billing Cycle - Updated for Current Session)
        // ------------------------------------------------------------------------
        //
        // 4. active: Set to true
        //    - Purpose: Re-enable renewal processing for this subscription
        //    - Previous State: false (set during cancellation)
        //    - New State: true (enables automated renewals to proceed)
        //
        // 5. next_renewal_ts: Current timestamp + period
        //    - Purpose: Schedule the next billing cycle from reactivation time
        //    - Calculation: reactivation_time + plan.period_secs
        //    - Example: Reactivated on 2024-06-01 with monthly plan → next_renewal_ts = 2024-07-01
        //
        // 6. last_amount: Current plan.price_usdc
        //    - Purpose: Track billing amount for upcoming renewals
        //    - Rationale: Plan pricing may have changed since cancellation
        //    - Example: Plan was $10/month, now $12/month → last_amount = $12
        //
        // 7. last_renewed_ts: Current timestamp
        //    - Purpose: Prevent immediate double-billing after reactivation
        //    - Security: Ensures renewals don't trigger immediately after reactivation
        //    - Validation: Renewal logic checks last_renewed_ts to prevent re-entry
        //
        // ------------------------------------------------------------------------
        // Off-Chain Integration Notes
        // ------------------------------------------------------------------------
        //
        // The SubscriptionReactivated event includes:
        // - total_renewals: Current renewals count (preserved from previous session)
        // - original_created_ts: Original creation timestamp
        //
        // Off-chain indexers should:
        // 1. Create a new session record when SubscriptionReactivated is emitted
        // 2. Store renewals_at_session_start = event.total_renewals
        // 3. Calculate current session renewals as: on_chain.renewals - renewals_at_session_start
        //
        // For detailed integration examples (TypeScript, SQL, GraphQL), see:
        // docs/SUBSCRIPTION_LIFECYCLE.md#off-chain-integration-guide

        subscription.active = true;
        subscription.next_renewal_ts = next_renewal_ts;
        subscription.last_amount = plan.price_usdc;
        subscription.last_renewed_ts = current_time;
        // Trials never apply to reactivations
        subscription.trial_ends_at = None;
        subscription.in_trial = false;
    } else {
        // NEW SUBSCRIPTION: Initialize all fields
        subscription.plan = plan.key();
        subscription.subscriber = ctx.accounts.subscriber.key();
        subscription.next_renewal_ts = next_renewal_ts;
        subscription.active = true;
        subscription.renewals = 0;
        subscription.created_ts = current_time;
        subscription.last_amount = plan.price_usdc;
        subscription.last_renewed_ts = current_time;
        subscription.bump = ctx.bumps.subscription;

        // Initialize trial fields
        if is_trial {
            subscription.trial_ends_at = Some(next_renewal_ts);
            subscription.in_trial = true;
        } else {
            subscription.trial_ends_at = None;
            subscription.in_trial = false;
        }
    }

    // Emit appropriate event based on whether this is a new subscription or reactivation
    if is_reactivation {
        emit!(crate::events::SubscriptionReactivated {
            merchant: merchant.key(),
            plan: plan.key(),
            subscriber: ctx.accounts.subscriber.key(),
            amount: plan.price_usdc,
            total_renewals: subscription.renewals,
            original_created_ts: subscription.created_ts,
        });
    } else if is_trial {
        // Emit TrialStarted event for new trial subscriptions
        emit!(crate::events::TrialStarted {
            subscription: subscription.key(),
            subscriber: ctx.accounts.subscriber.key(),
            plan: plan.key(),
            trial_ends_at: next_renewal_ts,
        });
    } else {
        // Emit Subscribed event for regular paid subscriptions
        emit!(Subscribed {
            merchant: merchant.key(),
            plan: plan.key(),
            subscriber: ctx.accounts.subscriber.key(),
            amount: plan.price_usdc,
        });
    }

    Ok(())
}
