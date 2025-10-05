use crate::errors::SubscriptionError;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct AdminWithdrawFeesArgs {
    pub amount: u64, // Amount in USDC microlamports to withdraw
}

#[derive(Accounts)]
pub struct AdminWithdrawFees<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(mut)]
    pub platform_authority: Signer<'info>,

    /// Platform treasury ATA where fees are currently stored
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub platform_treasury_ata: UncheckedAccount<'info>,

    /// Destination ATA where fees will be transferred
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub platform_destination_ata: UncheckedAccount<'info>,

    /// USDC mint account for validation
    /// CHECK: Validated as USDC mint in handler
    pub usdc_mint: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<AdminWithdrawFees>, args: AdminWithdrawFeesArgs) -> Result<()> {
    // Validate platform authority
    if ctx.accounts.platform_authority.key() != ctx.accounts.config.platform_authority {
        return Err(SubscriptionError::Unauthorized.into());
    }

    // Validate that platform_treasury_ata is the correct ATA derived from platform authority and USDC mint
    // This prevents the admin from withdrawing from arbitrary token accounts (e.g., merchant treasuries)
    let expected_platform_ata = get_associated_token_address(
        &ctx.accounts.config.platform_authority,
        ctx.accounts.usdc_mint.key,
    );

    if ctx.accounts.platform_treasury_ata.key() != expected_platform_ata {
        return Err(SubscriptionError::Unauthorized.into());
    }

    // Deserialize and validate token accounts with specific error handling
    let platform_treasury_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.platform_treasury_ata.data.borrow().as_ref(),
    )
    .map_err(|_| SubscriptionError::InvalidPlatformTreasuryAccount)?;

    let platform_destination_data: TokenAccount = TokenAccount::try_deserialize(
        &mut ctx.accounts.platform_destination_ata.data.borrow().as_ref(),
    )
    .map_err(|_| SubscriptionError::InvalidPlatformTreasuryAccount)?;

    let usdc_mint_data: Mint =
        Mint::try_deserialize(&mut ctx.accounts.usdc_mint.data.borrow().as_ref())
            .map_err(|_| SubscriptionError::InvalidUsdcMint)?;

    // Validate platform_treasury_ata is owned by platform_authority
    if platform_treasury_data.owner != ctx.accounts.platform_authority.key() {
        return Err(SubscriptionError::Unauthorized.into());
    }

    // Validate both ATAs use the correct USDC mint
    if platform_treasury_data.mint != ctx.accounts.usdc_mint.key()
        || platform_destination_data.mint != ctx.accounts.usdc_mint.key()
    {
        return Err(SubscriptionError::WrongMint.into());
    }

    // Validate sufficient balance
    if platform_treasury_data.amount < args.amount {
        return Err(SubscriptionError::InsufficientFunds.into());
    }

    // Validate amount is greater than 0
    if args.amount == 0 {
        return Err(SubscriptionError::InvalidPlan.into());
    }

    // Validate amount does not exceed configured maximum withdrawal limit
    // This prevents accidental or malicious drainage of entire treasury
    require!(
        args.amount <= ctx.accounts.config.max_withdrawal_amount,
        SubscriptionError::WithdrawLimitExceeded
    );

    // Transfer funds from platform treasury to destination
    let transfer_accounts = TransferChecked {
        from: ctx.accounts.platform_treasury_ata.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.platform_destination_ata.to_account_info(),
        authority: ctx.accounts.platform_authority.to_account_info(),
    };

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
        ),
        args.amount,
        usdc_mint_data.decimals,
    )?;

    Ok(())
}
