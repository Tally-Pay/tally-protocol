use crate::{
    constants::FEE_BASIS_POINTS_DIVISOR, errors::RecurringPaymentError, events::*, state::*,
    utils::validate_platform_treasury,
};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct ExecutePaymentArgs {
    // No args needed - renewal driven by executor
}

#[derive(Accounts)]
pub struct ExecutePayment<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump,
        constraint = !config.paused @ RecurringPaymentError::Inactive
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [b"payment_agreement", payment_terms.key().as_ref(), payment_agreement.payer.as_ref()],
        bump = payment_agreement.bump,
        constraint = payment_agreement.active @ RecurringPaymentError::Inactive
    )]
    pub payment_agreement: Account<'info, PaymentAgreement>,

    pub payment_terms: Account<'info, PaymentTerms>,

    #[account(
        seeds = [b"payee", payee.authority.as_ref()],
        bump = payee.bump
    )]
    pub payee: Account<'info, Payee>,

    // USDC accounts for transfers (same as start_subscription)
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub payer_usdc_ata: UncheckedAccount<'info>,

    /// CHECK: Validated as payee treasury ATA in handler
    #[account(mut)]
    pub payee_treasury_ata: UncheckedAccount<'info>,

    /// CHECK: Validated as platform treasury ATA in handler
    #[account(mut)]
    pub platform_treasury_ata: UncheckedAccount<'info>,

    /// Keeper (transaction caller) who executes the renewal
    #[account(mut)]
    pub executor: Signer<'info>,

    /// Keeper's USDC ATA where executor fee will be sent
    /// CHECK: Validated as executor's USDC token account in handler
    #[account(mut)]
    pub keeper_usdc_ata: UncheckedAccount<'info>,

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
}

#[allow(clippy::too_many_lines)]
pub fn handler(ctx: Context<ExecutePayment>, _args: ExecutePaymentArgs) -> Result<()> {
    let payment_agreement = &mut ctx.accounts.payment_agreement;
    let payment_terms = &ctx.accounts.payment_terms;
    let payee = &ctx.accounts.payee;

    // Get current timestamp
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // TRIAL TO PAID CONVERSION
    //
    // If this payment_agreement is in trial period, this renewal represents the first payment
    // after trial expiration. We need to:
    // 1. Clear the trial flags (in_trial = false, trial_ends_at = None)
    // 2. Process the payment normally
    // 3. Emit TrialConverted event after successful payment
    let was_trial = payment_agreement.in_trial;

    // Check timing: now >= next_renewal_ts AND now <= next_renewal_ts + grace_secs
    if current_time < payment_agreement.next_payment_ts {
        return Err(RecurringPaymentError::NotDue.into());
    }

    // Convert grace period to i64 with overflow check
    let grace_period_i64 = i64::try_from(payment_terms.grace_secs)
        .map_err(|_| RecurringPaymentError::ArithmeticError)?;

    // Calculate grace deadline with overflow check
    let grace_deadline = payment_agreement
        .next_payment_ts
        .checked_add(grace_period_i64)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    if current_time > grace_deadline {
        return Err(RecurringPaymentError::PastGrace.into());
    }

    // Prevent double-renewal attack: ensure sufficient time has passed since last renewal
    // This prevents multiple renewals within the same period
    let period_i64 =
        i64::try_from(payment_terms.period_secs).map_err(|_| RecurringPaymentError::ArithmeticError)?;
    let min_next_renewal_time = payment_agreement
        .last_payment_ts
        .checked_add(period_i64)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    if current_time < min_next_renewal_time {
        return Err(RecurringPaymentError::NotDue.into());
    }

    // Deserialize and validate token accounts with specific error handling
    let subscriber_ata_data: TokenAccount =
        TokenAccount::try_deserialize(&mut ctx.accounts.payer_usdc_ata.data.borrow().as_ref())
            .map_err(|_| RecurringPaymentError::InvalidSubscriberTokenAccount)?;

    let payee_treasury_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.payee_treasury_ata.data.borrow().as_ref(),
    )
    .map_err(|_| RecurringPaymentError::InvalidMerchantTreasuryAccount)?;

    let platform_treasury_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.platform_treasury_ata.data.borrow().as_ref(),
    )
    .map_err(|_| RecurringPaymentError::InvalidPlatformTreasuryAccount)?;

    let keeper_ata_data: TokenAccount =
        TokenAccount::try_deserialize(&mut ctx.accounts.keeper_usdc_ata.data.borrow().as_ref())
            .map_err(|_| RecurringPaymentError::InvalidSubscriberTokenAccount)?;

    let usdc_mint_data: Mint =
        Mint::try_deserialize(&mut ctx.accounts.usdc_mint.data.borrow().as_ref())
            .map_err(|_| RecurringPaymentError::InvalidUsdcMint)?;

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
    if subscriber_ata_data.owner != payment_agreement.payer {
        return Err(RecurringPaymentError::Unauthorized.into());
    }

    if keeper_ata_data.owner != ctx.accounts.executor.key() {
        return Err(RecurringPaymentError::Unauthorized.into());
    }

    if subscriber_ata_data.mint != payee.usdc_mint
        || payee_treasury_data.mint != payee.usdc_mint
        || platform_treasury_data.mint != payee.usdc_mint
        || keeper_ata_data.mint != payee.usdc_mint
    {
        return Err(RecurringPaymentError::WrongMint.into());
    }

    if ctx.accounts.usdc_mint.key() != payee.usdc_mint {
        return Err(RecurringPaymentError::WrongMint.into());
    }

    if ctx.accounts.payee_treasury_ata.key() != payee.treasury_ata {
        return Err(RecurringPaymentError::BadSeeds.into());
    }

    // Check delegate allowance for single-period renewal
    //
    // ALLOWANCE MANAGEMENT (Audit L-3):
    //
    // Renewals require only single-period allowance (>= payment_terms.amount_usdc), unlike
    // payment_agreement start which requires multi-period allowance (default 3x).
    //
    // This asymmetry is intentional to allow flexibility in allowance management.
    // However, we emit a LowAllowanceWarning event when allowance drops below
    // the recommended threshold (2x payment_terms price) to alert users and off-chain systems
    // to top up allowance before the next renewal cycle.
    //
    // This prevents the UX friction identified in audit finding L-3 where users
    // may successfully start subscriptions but encounter unexpected renewal failures
    // when allowance depletes.
    if subscriber_ata_data.delegated_amount < payment_terms.amount_usdc {
        return Err(RecurringPaymentError::InsufficientAllowance.into());
    }

    // Calculate recommended allowance threshold (2x payment_terms price)
    // Using checked arithmetic to prevent overflow
    let recommended_allowance = payment_terms
        .amount_usdc
        .checked_mul(2)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    // Emit warning event if allowance is sufficient for this renewal but below recommended threshold
    // This gives users and off-chain systems advance notice to top up allowance before next renewal
    if subscriber_ata_data.delegated_amount < recommended_allowance {
        emit!(crate::events::LowAllowanceWarning {
            payee: payee.key(),
            payment_terms: payment_terms.key(),
            payer: payment_agreement.payer,
            current_allowance: subscriber_ata_data.delegated_amount,
            recommended_allowance,
            plan_price: payment_terms.amount_usdc,
        });
    }

    // Explicitly validate PDA derivation to ensure the delegate PDA was derived with expected seeds
    let (expected_delegate_pda, _expected_bump) =
        Pubkey::find_program_address(&[b"delegate"], ctx.program_id);
    require!(
        ctx.accounts.program_delegate.key() == expected_delegate_pda,
        RecurringPaymentError::BadSeeds
    );

    // Detect delegate mismatch: Global delegate validation
    //
    // This protocol uses a single global delegate PDA for all merchants and subscriptions.
    // The global delegate enables users to subscribe to multiple merchants without delegate
    // conflicts (SPL Token limitation of one delegate per account).
    //
    // Detection logic:
    // - Check if the token account's current delegate matches our expected global delegate
    // - If mismatch detected, emit DelegateMismatchWarning event before failing
    // - This provides off-chain systems with actionable information about why renewal failed
    //
    // Possible causes of delegate mismatch:
    // - User revoked the delegate (intentional cancellation)
    // - User approved a different program's delegate (using account for other purposes)
    // - Token account delegate was never approved (should not happen for active subscriptions)
    let actual_delegate = Option::<Pubkey>::from(subscriber_ata_data.delegate);

    // Safe delegate validation without unwrap - use direct comparison
    if actual_delegate != Some(expected_delegate_pda) {
        // Emit warning event with diagnostic information
        emit!(crate::events::DelegateMismatchWarning {
            payee: payee.key(),
            payment_terms: payment_terms.key(),
            payer: payment_agreement.payer,
            expected_delegate: expected_delegate_pda,
            actual_delegate,
        });

        // Return unauthorized error (payment_agreement cannot renew with incorrect delegate)
        return Err(RecurringPaymentError::Unauthorized.into());
    }

    // Check sufficient funds
    if subscriber_ata_data.amount < payment_terms.amount_usdc {
        return Err(RecurringPaymentError::InsufficientFunds.into());
    }

    // Calculate executor fee first (deducted from total amount)
    let keeper_fee = u64::try_from(
        u128::from(payment_terms.amount_usdc)
            .checked_mul(u128::from(ctx.accounts.config.keeper_fee_bps))
            .ok_or(RecurringPaymentError::ArithmeticError)?
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .ok_or(RecurringPaymentError::ArithmeticError)?,
    )
    .map_err(|_| RecurringPaymentError::ArithmeticError)?;

    // Calculate remaining amount after executor fee
    let remaining_after_keeper = payment_terms
        .amount_usdc
        .checked_sub(keeper_fee)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    // Calculate platform fee from remaining amount (fee rate determined by payee's volume tier)
    let platform_fee = u64::try_from(
        u128::from(remaining_after_keeper)
            .checked_mul(u128::from(payee.volume_tier.platform_fee_bps()))
            .ok_or(RecurringPaymentError::ArithmeticError)?
            .checked_div(FEE_BASIS_POINTS_DIVISOR)
            .ok_or(RecurringPaymentError::ArithmeticError)?,
    )
    .map_err(|_| RecurringPaymentError::ArithmeticError)?;

    // Calculate payee amount from remaining amount
    let merchant_amount = remaining_after_keeper
        .checked_sub(platform_fee)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    // Prepare delegate signer seeds
    let delegate_bump = ctx.bumps.program_delegate;
    let delegate_seeds: &[&[&[u8]]] = &[&[b"delegate", &[delegate_bump]]];

    // Get USDC mint decimals from the mint account
    let usdc_decimals = usdc_mint_data.decimals;

    // Transfer payee amount to payee treasury (via delegate)
    if merchant_amount > 0 {
        let transfer_to_merchant = TransferChecked {
            from: ctx.accounts.payer_usdc_ata.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
            to: ctx.accounts.payee_treasury_ata.to_account_info(),
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
            from: ctx.accounts.payer_usdc_ata.to_account_info(),
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

    // Transfer executor fee to executor's ATA (via delegate)
    if keeper_fee > 0 {
        let transfer_to_keeper = TransferChecked {
            from: ctx.accounts.payer_usdc_ata.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
            to: ctx.accounts.keeper_usdc_ata.to_account_info(),
            authority: ctx.accounts.program_delegate.to_account_info(),
        };

        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_to_keeper,
                delegate_seeds,
            ),
            keeper_fee,
            usdc_decimals,
        )?;
    }

    // Update payment_agreement fields
    payment_agreement.next_payment_ts = payment_agreement
        .next_payment_ts
        .checked_add(period_i64)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    payment_agreement.payment_count = payment_agreement
        .payment_count
        .checked_add(1)
        .ok_or(RecurringPaymentError::ArithmeticError)?;

    payment_agreement.last_payment_amount = payment_terms.amount_usdc;
    payment_agreement.last_payment_ts = current_time;

    // Clear trial status if this was a trial conversion
    if was_trial {
                    }

    // Emit appropriate event based on whether this was a trial conversion or regular renewal
    if was_trial {
        // Emit TrialConverted event for trial to paid conversion
        emit!(crate::events::TrialConverted {
            payment_agreement: payment_agreement.key(),
            payer: payment_agreement.payer,
            payment_terms: payment_terms.key(),
        });
    }

    // Always emit PaymentExecuted event (regardless of trial status)
    emit!(PaymentExecuted {
        payee: payee.key(),
        payment_terms: payment_terms.key(),
        payer: payment_agreement.payer,
        amount: payment_terms.amount_usdc,
        executor: ctx.accounts.executor.key(),
        keeper_fee,
    });

    Ok(())
}
