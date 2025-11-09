use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::associated_token::{get_associated_token_address, AssociatedToken};
use anchor_spl::token::{spl_token::state::Account as TokenAccount, spl_token::state::Mint, Token};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitPayeeArgs {
    pub usdc_mint: Pubkey,
    pub treasury_ata: Pubkey,
}

#[derive(Accounts)]
#[instruction(args: InitPayeeArgs)]
pub struct InitPayee<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(
        init,
        payer = authority,
        space = crate::state::Payee::SPACE,
        seeds = [b"payee", authority.key().as_ref()],
        bump
    )]
    pub payee: Account<'info, crate::state::Payee>,

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

pub fn handler(ctx: Context<InitPayee>, args: InitPayeeArgs) -> Result<()> {
    // New payees start at Standard tier (0.25% platform fee)
    // Tier automatically upgrades based on 30-day rolling payment volume
    let default_tier = crate::state::VolumeTier::Standard;

    // Validate that the default tier fee is within config bounds
    default_tier.validate_fee()?;

    // Validate that the provided USDC mint matches the allowed mint in config
    // This prevents payees from using fake or arbitrary tokens
    require!(
        args.usdc_mint == ctx.accounts.config.allowed_mint,
        crate::errors::RecurringPaymentError::WrongMint
    );

    // Validate passed pubkeys match accounts
    require!(
        args.usdc_mint == ctx.accounts.usdc_mint.key(),
        crate::errors::RecurringPaymentError::WrongMint
    );
    require!(
        args.treasury_ata == ctx.accounts.treasury_ata.key(),
        crate::errors::RecurringPaymentError::WrongMint
    );

    // Validate USDC mint account
    let mint_data = ctx.accounts.usdc_mint.try_borrow_data()?;
    require!(
        mint_data.len() == Mint::LEN,
        crate::errors::RecurringPaymentError::WrongMint
    );
    require!(
        ctx.accounts.usdc_mint.owner == &ctx.accounts.token_program.key(),
        crate::errors::RecurringPaymentError::WrongMint
    );

    // Validate treasury ATA
    let ata_data = ctx.accounts.treasury_ata.try_borrow_data()?;
    require!(
        ata_data.len() == TokenAccount::LEN,
        crate::errors::RecurringPaymentError::WrongMint
    );
    require!(
        ctx.accounts.treasury_ata.owner == &ctx.accounts.token_program.key(),
        crate::errors::RecurringPaymentError::WrongMint
    );

    // Deserialize and validate ATA data
    let token_account = TokenAccount::unpack(&ata_data)?;
    require!(
        token_account.mint == args.usdc_mint,
        crate::errors::RecurringPaymentError::WrongMint
    );
    require!(
        token_account.owner == ctx.accounts.authority.key(),
        crate::errors::RecurringPaymentError::Unauthorized
    );

    // Validate that treasury_ata is the canonical Associated Token Account
    // derived from the payee authority and USDC mint.
    // This ensures compatibility with wallet integrations and off-chain indexing
    // that expect standard ATA addresses.
    let expected_treasury_ata = get_associated_token_address(
        &ctx.accounts.authority.key(),
        &args.usdc_mint,
    );
    require!(
        ctx.accounts.treasury_ata.key() == expected_treasury_ata,
        crate::errors::RecurringPaymentError::BadSeeds
    );

    let payee = &mut ctx.accounts.payee;

    // Get current timestamp for initialization and event
    let clock = Clock::get()?;

    payee.authority = ctx.accounts.authority.key();
    payee.usdc_mint = args.usdc_mint;
    payee.treasury_ata = args.treasury_ata;
    payee.volume_tier = default_tier;
    payee.monthly_volume_usdc = 0;
    payee.last_volume_update_ts = clock.unix_timestamp;
    payee.bump = ctx.bumps.payee;

    // Emit PayeeInitialized event
    emit!(crate::events::PayeeInitialized {
        payee: payee.key(),
        authority: ctx.accounts.authority.key(),
        usdc_mint: args.usdc_mint,
        treasury_ata: args.treasury_ata,
        volume_tier: default_tier,
        platform_fee_bps: default_tier.platform_fee_bps(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
