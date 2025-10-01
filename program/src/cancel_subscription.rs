use crate::{errors::SubscriptionError, events::*, state::*};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CancelSubscriptionArgs {
    // No args needed for cancellation
}

#[derive(Accounts)]
pub struct CancelSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", plan.key().as_ref(), subscriber.key().as_ref()],
        bump = subscription.bump,
        has_one = subscriber @ SubscriptionError::Unauthorized
    )]
    pub subscription: Account<'info, Subscription>,

    pub plan: Account<'info, Plan>,

    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    pub subscriber: Signer<'info>,
}

pub fn handler(ctx: Context<CancelSubscription>, _args: CancelSubscriptionArgs) -> Result<()> {
    let subscription = &mut ctx.accounts.subscription;
    let plan = &ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;

    // Make it idempotent - it's safe to "cancel" an already canceled subscription
    // No need to check if already canceled, just set active = false
    subscription.active = false;

    // Emit Canceled event
    emit!(Canceled {
        merchant: merchant.key(),
        plan: plan.key(),
        subscriber: ctx.accounts.subscriber.key(),
    });

    Ok(())
}
