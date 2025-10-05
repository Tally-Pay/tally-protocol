use crate::{constants::FEE_BASIS_POINTS_DIVISOR, errors::SubscriptionError, events::*, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct StartSubscriptionArgs {
    pub allowance_periods: u8, // Multiplier for allowance (default 3)
}

#[derive(Accounts)]
pub struct StartSubscription<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
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
        seeds = [b"delegate", merchant.key().as_ref()],
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

    // Validate platform treasury is owned by platform authority
    if platform_treasury_data.owner != ctx.accounts.config.platform_authority {
        return Err(SubscriptionError::Unauthorized.into());
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

    // Validate delegate allowance
    let required_allowance = plan
        .price_usdc
        .checked_mul(allowance_periods_u64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    if subscriber_ata_data.delegated_amount < required_allowance {
        return Err(SubscriptionError::InsufficientAllowance.into());
    }

    // Validate delegate is our program PDA
    if subscriber_ata_data.delegate.is_none()
        || subscriber_ata_data.delegate.unwrap() != ctx.accounts.program_delegate.key()
    {
        return Err(SubscriptionError::Unauthorized.into());
    }

    // Explicitly validate PDA derivation to ensure the delegate PDA was derived with expected seeds
    let (expected_delegate_pda, _expected_bump) =
        Pubkey::find_program_address(&[b"delegate", merchant.key().as_ref()], ctx.program_id);
    require!(
        ctx.accounts.program_delegate.key() == expected_delegate_pda,
        SubscriptionError::BadSeeds
    );

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
    let merchant_key = merchant.key();
    let delegate_bump = ctx.bumps.program_delegate;
    let delegate_seeds: &[&[&[u8]]] = &[&[b"delegate", merchant_key.as_ref(), &[delegate_bump]]];

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

    // Calculate next renewal timestamp
    let period_i64 =
        i64::try_from(plan.period_secs).map_err(|_| SubscriptionError::ArithmeticError)?;
    let next_renewal_ts = current_time
        .checked_add(period_i64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    // Update subscription account based on whether this is new or reactivation
    if is_reactivation {
        // REACTIVATION PATH: Preserve historical fields while resetting operational state
        //
        // When a previously canceled subscription is reactivated, we intentionally preserve
        // certain historical fields to maintain a complete record of the subscription's
        // lifetime across all sessions:
        //
        // PRESERVED FIELDS (not modified):
        //   - created_ts: Original subscription creation timestamp
        //   - renewals: Cumulative renewal count across all sessions (see state.rs documentation)
        //   - bump: PDA derivation seed (immutable)
        //
        // The renewals counter is deliberately preserved to track total renewals across
        // the entire subscription relationship, including previous sessions. This means
        // a subscription canceled after 10 renewals will show renewals=10 upon reactivation,
        // and will continue from 11 on the next renewal.
        //
        // RESET FIELDS (updated for new billing cycle):
        //   - active: Set to true to enable renewals
        //   - next_renewal_ts: Scheduled time for next billing cycle
        //   - last_amount: Current plan price (may differ from previous session)
        //   - last_renewed_ts: Current time to prevent immediate re-renewal

        subscription.active = true;
        subscription.next_renewal_ts = next_renewal_ts;
        subscription.last_amount = plan.price_usdc;
        subscription.last_renewed_ts = current_time;
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
    }

    // Emit Subscribed event
    emit!(Subscribed {
        merchant: merchant.key(),
        plan: plan.key(),
        subscriber: ctx.accounts.subscriber.key(),
        amount: plan.price_usdc,
    });

    Ok(())
}
