use anchor_lang::prelude::*;

// Example CLI command to initialize config:
// cargo run --package tally-cli -- init-config \
//   --platform-authority "YOUR_PLATFORM_AUTHORITY_PUBKEY" \
//   --max-platform-fee-bps 1000 \
//   --fee-basis-points-divisor 10000 \
//   --min-period-seconds 86400 \
//   --default-allowance-periods 3

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitConfigArgs {
    pub platform_authority: Pubkey,
    pub max_platform_fee_bps: u16,
    pub fee_basis_points_divisor: u16,
    pub min_period_seconds: u64,
    pub default_allowance_periods: u8,
}

#[derive(Accounts)]
pub struct InitConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = crate::state::Config::SPACE,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitConfig>, args: InitConfigArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.platform_authority = args.platform_authority;
    config.max_platform_fee_bps = args.max_platform_fee_bps;
    config.fee_basis_points_divisor = args.fee_basis_points_divisor;
    config.min_period_seconds = args.min_period_seconds;
    config.default_allowance_periods = args.default_allowance_periods;
    config.bump = ctx.bumps.config;

    Ok(())
}
