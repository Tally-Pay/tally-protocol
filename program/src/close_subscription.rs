use crate::{errors::SubscriptionError, events::*, state::*};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CloseSubscriptionArgs {
    // No args needed for closing
}

#[derive(Accounts)]
pub struct CloseSubscription<'info> {
    #[account(
        mut,
        seeds = [b"subscription", subscription.plan.as_ref(), subscriber.key().as_ref()],
        bump = subscription.bump,
        has_one = subscriber @ SubscriptionError::Unauthorized,
        constraint = !subscription.active @ SubscriptionError::AlreadyActive,
        close = subscriber
    )]
    pub subscription: Account<'info, Subscription>,

    #[account(mut)]
    pub subscriber: Signer<'info>,
}

pub fn handler(ctx: Context<CloseSubscription>, _args: CloseSubscriptionArgs) -> Result<()> {
    let subscription = &ctx.accounts.subscription;

    // Emit SubscriptionClosed event before account is closed
    emit!(SubscriptionClosed {
        plan: subscription.plan,
        subscriber: ctx.accounts.subscriber.key(),
    });

    // The `close` constraint in the Accounts struct will:
    // 1. Transfer all lamports (rent) to subscriber account
    // 2. Set subscription account data to all zeros
    // 3. Set subscription account owner to System Program
    // This is handled automatically by Anchor after handler returns Ok(())

    Ok(())
}
