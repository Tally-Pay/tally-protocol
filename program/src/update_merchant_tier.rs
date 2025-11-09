use anchor_lang::prelude::*;

use crate::state::VolumeTier;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateMerchantTierArgs {
    pub new_tier: VolumeTier,
}

#[derive(Accounts)]
pub struct UpdateMerchantTier<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, crate::state::Config>,

    /// Merchant account to update
    #[account(
        mut,
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, crate::state::Merchant>,

    /// Authority signer (either merchant authority OR platform authority)
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<UpdateMerchantTier>, args: UpdateMerchantTierArgs) -> Result<()> {
    let merchant = &mut ctx.accounts.merchant;
    let config = &ctx.accounts.config;

    // Validate authority: must be either merchant authority OR platform authority
    require!(
        ctx.accounts.authority.key() == merchant.authority
            || ctx.accounts.authority.key() == config.platform_authority,
        crate::errors::SubscriptionError::Unauthorized
    );

    // Store old tier for event
    let old_tier = merchant.volume_tier;

    // Validate new tier fee is within config bounds
    args.new_tier.validate_fee()?;

    // Update merchant tier (fee is derived from tier)
    merchant.volume_tier = args.new_tier;

    // Emit event for audit trail
    emit!(crate::events::VolumeTierUpgraded {
        merchant: merchant.key(),
        old_tier,
        new_tier: args.new_tier,
        monthly_volume_usdc: merchant.monthly_volume_usdc,
        new_platform_fee_bps: args.new_tier.platform_fee_bps(),
    });

    Ok(())
}
