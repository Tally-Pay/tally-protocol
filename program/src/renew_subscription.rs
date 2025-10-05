use crate::{constants::FEE_BASIS_POINTS_DIVISOR, errors::SubscriptionError, events::*, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct RenewSubscriptionArgs {
    // No args needed - renewal driven by keeper
}

#[derive(Accounts)]
pub struct RenewSubscription<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump,
        constraint = !config.paused @ SubscriptionError::Inactive
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [b"subscription", plan.key().as_ref(), subscription.subscriber.as_ref()],
        bump = subscription.bump,
        constraint = subscription.active @ SubscriptionError::Inactive
    )]
    pub subscription: Account<'info, Subscription>,

    pub plan: Account<'info, Plan>,

    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    // USDC accounts for transfers (same as start_subscription)
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
}

#[allow(clippy::too_many_lines)]
pub fn handler(ctx: Context<RenewSubscription>, _args: RenewSubscriptionArgs) -> Result<()> {
    let subscription = &mut ctx.accounts.subscription;
    let plan = &ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;

    // Get current timestamp
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // Check timing: now >= next_renewal_ts AND now <= next_renewal_ts + grace_secs
    if current_time < subscription.next_renewal_ts {
        return Err(SubscriptionError::NotDue.into());
    }

    // Convert grace period to i64 with overflow check
    let grace_period_i64 = i64::try_from(plan.grace_secs)
        .map_err(|_| SubscriptionError::ArithmeticError)?;

    // Calculate grace deadline with overflow check
    let grace_deadline = subscription
        .next_renewal_ts
        .checked_add(grace_period_i64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    if current_time > grace_deadline {
        return Err(SubscriptionError::PastGrace.into());
    }

    // Prevent double-renewal attack: ensure sufficient time has passed since last renewal
    // This prevents multiple renewals within the same period
    let period_i64 =
        i64::try_from(plan.period_secs).map_err(|_| SubscriptionError::ArithmeticError)?;
    let min_next_renewal_time = subscription
        .last_renewed_ts
        .checked_add(period_i64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    if current_time < min_next_renewal_time {
        return Err(SubscriptionError::NotDue.into());
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
    if subscriber_ata_data.owner != subscription.subscriber {
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

    // Check delegate allowance for single-period renewal
    //
    // ALLOWANCE MANAGEMENT (Audit L-3):
    //
    // Renewals require only single-period allowance (>= plan.price_usdc), unlike
    // subscription start which requires multi-period allowance (default 3x).
    //
    // This asymmetry is intentional to allow flexibility in allowance management.
    // However, we emit a LowAllowanceWarning event when allowance drops below
    // the recommended threshold (2x plan price) to alert users and off-chain systems
    // to top up allowance before the next renewal cycle.
    //
    // This prevents the UX friction identified in audit finding L-3 where users
    // may successfully start subscriptions but encounter unexpected renewal failures
    // when allowance depletes.
    if subscriber_ata_data.delegated_amount < plan.price_usdc {
        return Err(SubscriptionError::InsufficientAllowance.into());
    }

    // Calculate recommended allowance threshold (2x plan price)
    // Using checked arithmetic to prevent overflow
    let recommended_allowance = plan
        .price_usdc
        .checked_mul(2)
        .ok_or(SubscriptionError::ArithmeticError)?;

    // Emit warning event if allowance is sufficient for this renewal but below recommended threshold
    // This gives users and off-chain systems advance notice to top up allowance before next renewal
    if subscriber_ata_data.delegated_amount < recommended_allowance {
        emit!(crate::events::LowAllowanceWarning {
            merchant: merchant.key(),
            plan: plan.key(),
            subscriber: subscription.subscriber,
            current_allowance: subscriber_ata_data.delegated_amount,
            recommended_allowance,
            plan_price: plan.price_usdc,
        });
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

    // Check sufficient funds
    if subscriber_ata_data.amount < plan.price_usdc {
        return Err(SubscriptionError::InsufficientFunds.into());
    }

    // Calculate platform fee using checked arithmetic (same as start_subscription)
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

    // Update subscription fields
    subscription.next_renewal_ts = subscription
        .next_renewal_ts
        .checked_add(period_i64)
        .ok_or(SubscriptionError::ArithmeticError)?;

    subscription.renewals = subscription
        .renewals
        .checked_add(1)
        .ok_or(SubscriptionError::ArithmeticError)?;

    subscription.last_amount = plan.price_usdc;
    subscription.last_renewed_ts = current_time;

    // Emit Renewed event
    emit!(Renewed {
        merchant: merchant.key(),
        plan: plan.key(),
        subscriber: subscription.subscriber,
        amount: plan.price_usdc,
    });

    Ok(())
}
