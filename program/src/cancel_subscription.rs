use crate::{errors::SubscriptionError, events::*, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Revoke, Token, TokenAccount};

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

    /// Subscriber's USDC token account where delegate approval will be revoked
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub subscriber_usdc_ata: UncheckedAccount<'info>,

    /// Program PDA that acts as delegate - used to validate delegate identity before revocation
    /// CHECK: PDA derived from program, validated in handler
    #[account(
        seeds = [b"delegate", merchant.key().as_ref()],
        bump
    )]
    pub program_delegate: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<CancelSubscription>, _args: CancelSubscriptionArgs) -> Result<()> {
    let subscription = &mut ctx.accounts.subscription;
    let plan = &ctx.accounts.plan;
    let merchant = &ctx.accounts.merchant;

    // Deserialize and validate subscriber's token account
    let subscriber_ata_data: TokenAccount =
        TokenAccount::try_deserialize(&mut ctx.accounts.subscriber_usdc_ata.data.borrow().as_ref())
            .map_err(|_| SubscriptionError::InvalidSubscriberTokenAccount)?;

    // Validate token account ownership and mint
    if subscriber_ata_data.owner != ctx.accounts.subscriber.key() {
        return Err(SubscriptionError::Unauthorized.into());
    }

    if subscriber_ata_data.mint != merchant.usdc_mint {
        return Err(SubscriptionError::WrongMint.into());
    }

    // Validate program delegate PDA derivation to ensure correct delegate account
    let (expected_delegate_pda, _expected_bump) =
        Pubkey::find_program_address(&[b"delegate", merchant.key().as_ref()], ctx.program_id);
    require!(
        ctx.accounts.program_delegate.key() == expected_delegate_pda,
        SubscriptionError::BadSeeds
    );

    // Revoke delegate approval to prevent further renewals
    //
    // IMPORTANT - SPL Token Single-Delegate Limitation (M-3):
    //
    // SPL Token accounts support only ONE delegate at a time. This means:
    // 1. Revoking the delegate here affects ALL subscriptions that use this token account
    // 2. If the user has subscriptions with multiple merchants, this revocation will
    //    make ALL of those subscriptions non-functional, not just this one
    // 3. Other merchants' subscriptions will appear active but cannot renew
    //
    // This is a FUNDAMENTAL ARCHITECTURAL LIMITATION of SPL Token, not a bug.
    // See docs/MULTI_MERCHANT_LIMITATION.md for:
    // - Detailed explanation of the limitation
    // - Workarounds (per-merchant token accounts)
    // - Future migration paths (Token-2022, global delegate)
    //
    // We only revoke if the current delegate matches our program's delegate PDA.
    // This prevents revoking unrelated delegations to other programs.
    if let Some(current_delegate) = Option::<Pubkey>::from(subscriber_ata_data.delegate) {
        if current_delegate == expected_delegate_pda {
            let revoke_accounts = Revoke {
                source: ctx.accounts.subscriber_usdc_ata.to_account_info(),
                authority: ctx.accounts.subscriber.to_account_info(),
            };

            token::revoke(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                revoke_accounts,
            ))?;
        }
        // If delegate is not ours, skip revocation (already revoked or delegated elsewhere)
    }

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
