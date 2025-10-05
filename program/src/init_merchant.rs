use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{spl_token::state::Account as TokenAccount, spl_token::state::Mint, Token};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitMerchantArgs {
    pub usdc_mint: Pubkey,
    pub treasury_ata: Pubkey,
    pub platform_fee_bps: u16,
}

#[derive(Accounts)]
#[instruction(args: InitMerchantArgs)]
pub struct InitMerchant<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(
        init,
        payer = authority,
        space = crate::state::Merchant::SPACE,
        seeds = [b"merchant", authority.key().as_ref()],
        bump
    )]
    pub merchant: Account<'info, crate::state::Merchant>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// USDC mint account - will be validated in handler
    /// CHECK: Validated as USDC mint in handler logic
    pub usdc_mint: UncheckedAccount<'info>,

    /// Treasury ATA for receiving merchant fees - will be validated in handler
    /// CHECK: Validated as ATA for authority & `usdc_mint` in handler logic
    pub treasury_ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitMerchant>, args: InitMerchantArgs) -> Result<()> {
    // Validate platform fee is within acceptable range using config
    require!(
        args.platform_fee_bps >= ctx.accounts.config.min_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidPlan
    );
    require!(
        args.platform_fee_bps <= ctx.accounts.config.max_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidPlan
    );

    // Validate passed pubkeys match accounts
    require!(
        args.usdc_mint == ctx.accounts.usdc_mint.key(),
        crate::errors::SubscriptionError::WrongMint
    );
    require!(
        args.treasury_ata == ctx.accounts.treasury_ata.key(),
        crate::errors::SubscriptionError::WrongMint
    );

    // Validate USDC mint account
    let mint_data = ctx.accounts.usdc_mint.try_borrow_data()?;
    require!(
        mint_data.len() == Mint::LEN,
        crate::errors::SubscriptionError::WrongMint
    );
    require!(
        ctx.accounts.usdc_mint.owner == &ctx.accounts.token_program.key(),
        crate::errors::SubscriptionError::WrongMint
    );

    // Validate treasury ATA
    let ata_data = ctx.accounts.treasury_ata.try_borrow_data()?;
    require!(
        ata_data.len() == TokenAccount::LEN,
        crate::errors::SubscriptionError::WrongMint
    );
    require!(
        ctx.accounts.treasury_ata.owner == &ctx.accounts.token_program.key(),
        crate::errors::SubscriptionError::WrongMint
    );

    // Deserialize and validate ATA data
    let token_account = TokenAccount::unpack(&ata_data)?;
    require!(
        token_account.mint == args.usdc_mint,
        crate::errors::SubscriptionError::WrongMint
    );
    require!(
        token_account.owner == ctx.accounts.authority.key(),
        crate::errors::SubscriptionError::Unauthorized
    );

    let merchant = &mut ctx.accounts.merchant;
    merchant.authority = ctx.accounts.authority.key();
    merchant.usdc_mint = args.usdc_mint;
    merchant.treasury_ata = args.treasury_ata;
    merchant.platform_fee_bps = args.platform_fee_bps;
    merchant.bump = ctx.bumps.merchant;

    Ok(())
}
