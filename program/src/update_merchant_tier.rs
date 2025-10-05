use anchor_lang::prelude::*;

use crate::state::MerchantTier;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateMerchantTierArgs {
    pub new_tier: MerchantTier,
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
    let old_tier = merchant.tier;

    // Calculate new fee based on tier
    let new_fee_bps = args.new_tier.fee_bps();

    // Validate fee is within config bounds
    require!(
        new_fee_bps >= config.min_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidConfiguration
    );
    require!(
        new_fee_bps <= config.max_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidConfiguration
    );

    // Update merchant tier and fee
    merchant.tier = args.new_tier;
    merchant.platform_fee_bps = new_fee_bps;

    // Emit event for audit trail
    emit!(crate::events::MerchantTierChanged {
        merchant: merchant.key(),
        old_tier,
        new_tier: args.new_tier,
        new_fee_bps,
    });

    Ok(())
}
