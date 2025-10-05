use crate::{errors::SubscriptionError, events::*, state::*};
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
        init,
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

    // Use default from config if allowance_periods is 0
    let allowance_periods = if args.allowance_periods == 0 {
        ctx.accounts.config.default_allowance_periods
    } else {
        args.allowance_periods
    };

    // Get current time
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // Validate delegate allowance
    let required_allowance = plan
        .price_usdc
        .checked_mul(u64::from(allowance_periods))
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

    // Calculate platform fee using checked arithmetic
    let platform_fee = u64::try_from(
        u128::from(plan.price_usdc)
            .checked_mul(u128::from(merchant.platform_fee_bps))
            .ok_or(SubscriptionError::ArithmeticError)?
            .checked_div(u128::from(ctx.accounts.config.fee_basis_points_divisor))
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

    // Initialize subscription account
    subscription.plan = plan.key();
    subscription.subscriber = ctx.accounts.subscriber.key();
    subscription.next_renewal_ts = next_renewal_ts;
    subscription.active = true;
    subscription.renewals = 0;
    subscription.created_ts = current_time;
    subscription.last_amount = plan.price_usdc;
    subscription.last_renewed_ts = current_time;
    subscription.bump = ctx.bumps.subscription;

    // Emit Subscribed event
    emit!(Subscribed {
        merchant: merchant.key(),
        plan: plan.key(),
        subscriber: ctx.accounts.subscriber.key(),
        amount: plan.price_usdc,
    });

    Ok(())
}
