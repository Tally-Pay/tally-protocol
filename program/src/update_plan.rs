use crate::errors::SubscriptionError;
use crate::events::PlanStatusChanged;
use crate::state::{Config, Merchant, Plan};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdatePlanArgs {
    /// New active status for the plan
    pub active: bool,
}

#[derive(Accounts)]
pub struct UpdatePlan<'info> {
    /// Global configuration account for platform authority validation
    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// Plan account to update
    #[account(
        mut,
        seeds = [b"plan", merchant.key().as_ref(), plan.plan_id.as_ref()],
        bump,
        has_one = merchant
    )]
    pub plan: Account<'info, Plan>,

    /// Merchant account that owns the plan
    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    /// Authority signing the transaction (either merchant authority or platform admin)
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<UpdatePlan>, args: UpdatePlanArgs) -> Result<()> {
    let plan = &mut ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;
    let config = &ctx.accounts.config;

    // Validate authority: either merchant authority OR platform admin
    let is_merchant_authority = ctx.accounts.authority.key() == merchant.authority;
    let is_platform_admin = ctx.accounts.authority.key() == config.platform_authority;

    require!(
        is_merchant_authority || is_platform_admin,
        SubscriptionError::Unauthorized
    );

    // Determine who changed the status for event emission
    let changed_by = if is_platform_admin {
        "platform"
    } else {
        "merchant"
    };

    // Update plan status
    plan.active = args.active;

    // Emit status change event
    emit!(PlanStatusChanged {
        merchant: merchant.key(),
        plan: plan.key(),
        active: args.active,
        changed_by: changed_by.to_string(),
    });

    msg!(
        "Plan status updated: plan={}, active={}, changed_by={}",
        plan.key(),
        args.active,
        changed_by
    );

    Ok(())
}
